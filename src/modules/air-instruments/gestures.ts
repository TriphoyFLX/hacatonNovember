import { isFist } from '@/modules/vision/handMetrics';
import { FINGER_NAMES, type FingerName, type TrackedHand } from '@/modules/vision/types';
import { toneEngine } from './audio/toneEngine';
import { mirrorX } from './coords';
import { getVoicing, GUITAR_STRING_YS, type GuitarTone } from './instruments/guitarVoicings';
import type {
  CircleZone,
  GuitarPlayState,
  HitEvent,
  InstrumentId,
  InstrumentSwitchEvent,
  ZoneRect,
} from './types';
import { pointInCircle, pointInRect } from './zones';

/** Preferred string bands per finger (classical PIMA-ish feel on 6 strings). */
const FINGER_STRING_PREF: Record<FingerName, number[]> = {
  thumb: [0, 1, 2],
  index: [2, 3],
  middle: [3, 4],
  ring: [4, 5],
  pinky: [4, 5],
};

const FINGER_COOLDOWN_MS = 55;
const STRING_HIT_TOLERANCE = 0.052;

export interface GestureContext {
  instrument: InstrumentId;
  sensitivity: number;
  muted: boolean;
  pianoZones: ZoneRect[];
  drumZones: CircleZone[];
  chordZones: ZoneRect[];
  activeChord: string;
  guitarTone: GuitarTone;
}

interface ActiveNote {
  note: string;
  handId: number;
  finger: 'index' | 'middle';
}

export class GestureMapper {
  private activeNotes = new Map<string, ActiveNote>();
  private drumCooldown = new Map<string, number>();
  private lastStrumAt = 0;
  private strumPrevY = new Map<number, number>();
  private pluckPrevY = new Map<string, number>();
  private fingerCooldown = new Map<string, number>();
  private hits: HitEvent[] = [];
  private switches: InstrumentSwitchEvent[] = [];
  private selectedChord = 'Em';
  private lastSwitchAt = 0;
  private swipeArmed = true;
  private guitarState: GuitarPlayState = {
    chord: 'Em',
    muted: false,
    power: false,
    direction: null,
    stringEnergy: [0, 0, 0, 0, 0, 0],
    pickX: 0.6,
    pickY: 0.48,
    fingers: [],
  };

  consumeHits(): HitEvent[] {
    const next = this.hits;
    this.hits = [];
    return next;
  }

  consumeInstrumentSwitches(): InstrumentSwitchEvent[] {
    const next = this.switches;
    this.switches = [];
    return next;
  }

  getActiveNoteLabels(): string[] {
    return [...this.activeNotes.values()].map((item) => item.note);
  }

  getSelectedChord(): string {
    return this.selectedChord;
  }

  getGuitarState(): GuitarPlayState {
    return {
      ...this.guitarState,
      stringEnergy: [...this.guitarState.stringEnergy],
      fingers: this.guitarState.fingers.map((finger) => ({ ...finger })),
    };
  }

  process(hands: TrackedHand[], ctx: GestureContext, now = performance.now()): void {
    const switched = this.processTwoHandSwipe(hands, ctx, now);
    if (switched) {
      // Avoid accidental hits during the switch swipe.
      this.releaseAllNotes(ctx.muted);
      this.releaseMissingHands(hands);
      return;
    }

    if (ctx.instrument === 'piano') {
      this.processPiano(hands, ctx);
    } else if (ctx.instrument === 'drums') {
      this.processDrums(hands, ctx, now);
    } else {
      this.processGuitar(hands, ctx, now);
    }

    this.releaseMissingHands(hands);
  }

  panic(): void {
    this.activeNotes.clear();
    this.drumCooldown.clear();
    this.strumPrevY.clear();
    this.switches = [];
    this.swipeArmed = true;
    toneEngine.panic();
  }

  private processTwoHandSwipe(
    hands: TrackedHand[],
    ctx: GestureContext,
    now: number,
  ): boolean {
    if (hands.length < 2) {
      this.swipeArmed = true;
      return false;
    }

    if (now - this.lastSwitchAt < 700) {
      return false;
    }

    const [left, right] =
      mirrorX(hands[0].palm).x <= mirrorX(hands[1].palm).x
        ? [hands[0], hands[1]]
        : [hands[1], hands[0]];

    const leftPalm = mirrorX(left.palm);
    const rightPalm = mirrorX(right.palm);
    const separation = rightPalm.x - leftPalm.x;

    // Both hands in front of camera and clearly apart.
    if (separation < 0.14) {
      this.swipeArmed = true;
      return false;
    }

    // Upward wave: both wrists move up (negative y in image space).
    const leftVy = left.tipVelocity.wrist.y;
    const rightVy = right.tipVelocity.wrist.y;
    const upThreshold = -0.95 / Math.max(0.4, ctx.sensitivity);
    const bothUp = leftVy < upThreshold && rightVy < upThreshold;

    // Re-arm when hands settle / move down slowly.
    if (!bothUp) {
      if (leftVy > upThreshold * 0.35 && rightVy > upThreshold * 0.35) {
        this.swipeArmed = true;
      }
      return false;
    }

    if (!this.swipeArmed) {
      return false;
    }

    this.swipeArmed = false;
    this.lastSwitchAt = now;
    // Up-wave always advances to the next instrument.
    this.switches.push({
      direction: 'next',
      at: now,
    });
    return true;
  }

  private processPiano(hands: TrackedHand[], ctx: GestureContext): void {
    const seen = new Set<string>();
    const threshold = 0.55 / Math.max(0.35, ctx.sensitivity);

    for (const hand of hands) {
      const fingers: Array<{ finger: 'index' | 'middle'; tip: TrackedHand['tips']['index']; vy: number }> =
        [
          { finger: 'index', tip: hand.tips.index, vy: hand.tipVelocity.index.y },
          { finger: 'middle', tip: hand.tips.middle, vy: hand.tipVelocity.middle.y },
        ];

      for (const { finger, tip, vy } of fingers) {
        const mirrored = mirrorX(tip);
        const zone = ctx.pianoZones.find((item) => pointInRect(mirrored.x, mirrored.y, item));
        if (!zone) {
          continue;
        }

        const key = `${hand.id}:${finger}`;
        seen.add(key);
        const note = zone.label;
        const existing = this.activeNotes.get(key);

        if (!existing && vy > threshold) {
          const velocity = Math.min(1, 0.25 + vy / 3.5);
          if (!ctx.muted) {
            toneEngine.pianoAttack(note, velocity);
          }
          this.activeNotes.set(key, { note, handId: hand.id, finger });
          this.hits.push({
            id: `${key}:${performance.now()}`,
            instrument: 'piano',
            label: note,
            x: mirrored.x,
            y: mirrored.y,
            velocity,
            color: zone.color,
            at: performance.now(),
          });
        } else if (existing && existing.note !== note) {
          if (!ctx.muted) {
            toneEngine.pianoRelease(existing.note);
            toneEngine.pianoAttack(note, 0.55);
          }
          this.activeNotes.set(key, { note, handId: hand.id, finger });
        }
      }
    }

    for (const [key, active] of [...this.activeNotes.entries()]) {
      if (!seen.has(key)) {
        if (!ctx.muted) {
          toneEngine.pianoRelease(active.note);
        }
        this.activeNotes.delete(key);
      }
    }
  }

  private processDrums(hands: TrackedHand[], ctx: GestureContext, now: number): void {
    const threshold = 1.1 / Math.max(0.35, ctx.sensitivity);

    for (const hand of hands) {
      const speed = Math.hypot(hand.tipVelocity.wrist.x, hand.tipVelocity.wrist.y);
      if (hand.tipVelocity.wrist.y < threshold && speed < threshold) {
        continue;
      }

      const palm = mirrorX(hand.palm);
      const zone = ctx.drumZones.find((item) => pointInCircle(palm.x, palm.y, item));
      if (!zone) {
        continue;
      }

      const last = this.drumCooldown.get(zone.id) ?? 0;
      if (now - last < 120) {
        continue;
      }

      this.drumCooldown.set(zone.id, now);
      const velocity = Math.min(1, 0.3 + speed / 4);
      const pad = zone.id.replace('drums:', '') as 'kick' | 'snare' | 'hat' | 'tom' | 'crash';
      if (!ctx.muted) {
        toneEngine.playDrum(pad, velocity);
      }
      this.hits.push({
        id: `${zone.id}:${now}`,
        instrument: 'drums',
        label: zone.label,
        x: zone.x,
        y: zone.y,
        velocity,
        color: zone.color,
        at: now,
      });
    }
  }

  private processGuitar(hands: TrackedHand[], ctx: GestureContext, now: number): void {
    this.guitarState.stringEnergy = this.guitarState.stringEnergy.map((value) => value * 0.92);

    const fretting = hands.find((hand) => mirrorX(hand.palm).x < 0.34) ?? null;
    const picking =
      hands.find((hand) => mirrorX(hand.palm).x >= 0.34) ??
      (hands.length === 1 ? hands[0] : null);

    let chord = this.selectedChord || ctx.activeChord;
    let muted = false;
    let power = false;

    if (fretting) {
      const palm = mirrorX(fretting.palm);
      const zone = ctx.chordZones.find((item) => pointInRect(palm.x, palm.y, item));
      if (zone) {
        chord = zone.label;
        this.selectedChord = zone.label;
      }
      muted = isFist(fretting, 0.36);
      power = muted;
    }

    this.guitarState.chord = chord;
    this.guitarState.muted = muted;
    this.guitarState.power = power;
    this.guitarState.fingers = [];
    this.guitarState.direction = null;

    const voicing = getVoicing(chord);
    const notes = power ? voicing.power : voicing.strings;

    if (!picking) {
      return;
    }

    const palm = mirrorX(picking.palm);
    const inNeck = palm.x > 0.22 && palm.x < 0.98 && palm.y > 0.2 && palm.y < 0.82;

    // Track all five fingertips over the neck.
    const fingerCursors = FINGER_NAMES.map((finger) => {
      const tip = mirrorX(picking.tips[finger]);
      return {
        finger,
        x: tip.x,
        y: tip.y,
        active: inNeck && tip.x > 0.24 && tip.x < 0.96,
      };
    });
    this.guitarState.fingers = fingerCursors;
    const lead = fingerCursors.find((item) => item.finger === 'index') ?? fingerCursors[0];
    this.guitarState.pickX = lead.x;
    this.guitarState.pickY = lead.y;

    if (!inNeck) {
      this.strumPrevY.set(picking.id, palm.y);
      for (const finger of FINGER_NAMES) {
        this.pluckPrevY.delete(`${picking.id}:${finger}`);
      }
      return;
    }

    // Full strum when palm/wrist sweeps fast across the neck.
    const prevY = this.strumPrevY.get(picking.id);
    this.strumPrevY.set(picking.id, palm.y);
    const wristSpeed = Math.abs(picking.tipVelocity.wrist.y);
    const strumThreshold = 0.03 / Math.max(0.35, ctx.sensitivity);
    const strumMinSpeed = 1.15 / ctx.sensitivity;

    if (
      prevY !== undefined &&
      Math.abs(palm.y - prevY) > strumThreshold &&
      wristSpeed > strumMinSpeed &&
      now - this.lastStrumAt > 160
    ) {
      this.lastStrumAt = now;
      const direction: 'down' | 'up' = palm.y - prevY > 0 ? 'down' : 'up';
      const velocity = Math.min(1, 0.28 + wristSpeed / 3.1);
      this.guitarState.direction = direction;

      if (!ctx.muted) {
        toneEngine.strumGuitar({
          notes,
          direction,
          velocity,
          muted,
          tone: ctx.guitarTone,
        });
      }

      notes.forEach((note, index) => {
        if (note) {
          this.guitarState.stringEnergy[index] = Math.max(
            this.guitarState.stringEnergy[index],
            velocity * (direction === 'down' ? 1 - index * 0.05 : 0.75 + index * 0.04),
          );
        }
      });

      this.hits.push({
        id: `strum:${direction}:${now}`,
        instrument: 'guitar',
        label: `${chord}${muted ? ' mute' : ''} · ${direction}`,
        x: palm.x,
        y: palm.y,
        velocity,
        color: voicing.color,
        at: now,
      });
      return;
    }

    // Fingerstyle: each fingertip independently plucks strings it crosses.
    const minFingerSpeed = 0.2 / Math.max(0.35, ctx.sensitivity);
    const maxFingerSpeed = 4.2 / Math.max(0.35, ctx.sensitivity);

    for (const finger of FINGER_NAMES) {
      const tip = mirrorX(picking.tips[finger]);
      if (tip.x < 0.24 || tip.x > 0.96 || tip.y < 0.22 || tip.y > 0.78) {
        this.pluckPrevY.delete(`${picking.id}:${finger}`);
        continue;
      }

      const tipKey = `${picking.id}:${finger}`;
      const prevTipY = this.pluckPrevY.get(tipKey);
      this.pluckPrevY.set(tipKey, tip.y);

      const tipSpeed = Math.hypot(
        picking.tipVelocity[finger].x,
        picking.tipVelocity[finger].y,
      );
      if (prevTipY === undefined || tipSpeed < minFingerSpeed || tipSpeed > maxFingerSpeed) {
        continue;
      }

      const cooldownKey = `${tipKey}:hit`;
      const lastHit = this.fingerCooldown.get(cooldownKey) ?? 0;
      if (now - lastHit < FINGER_COOLDOWN_MS) {
        continue;
      }

      const crossed = this.findCrossedString(prevTipY, tip.y, finger);
      if (crossed < 0 || !notes[crossed]) {
        continue;
      }

      this.fingerCooldown.set(cooldownKey, now);
      const registerBoost = crossed <= 1 ? 1.12 : crossed >= 4 ? 1.08 : 1;
      const velocity = Math.min(1, (0.34 + tipSpeed / 2.3) * registerBoost);
      if (!ctx.muted) {
        toneEngine.pluckGuitar({
          note: notes[crossed] as string,
          velocity,
          stringIndex: crossed,
          muted,
          tone: ctx.guitarTone,
        });
      }
      this.guitarState.stringEnergy[crossed] = Math.max(
        this.guitarState.stringEnergy[crossed],
        velocity,
      );
      this.hits.push({
        id: `pluck:${finger}:${crossed}:${now}`,
        instrument: 'guitar',
        label: `${chord} · ${fingerShort(finger)}${crossed + 1}`,
        x: tip.x,
        y: GUITAR_STRING_YS[crossed],
        velocity,
        color: voicing.color,
        at: now,
      });
    }
  }

  private findCrossedString(prevY: number, nextY: number, finger: FingerName): number {
    const preferred = FINGER_STRING_PREF[finger];
    const candidates: number[] = [];

    GUITAR_STRING_YS.forEach((stringY, index) => {
      const crossed =
        (prevY < stringY && nextY >= stringY) || (prevY >= stringY && nextY < stringY);
      if (!crossed) {
        return;
      }
      // Soft proximity gate so a noisy jump doesn't fire distant strings.
      const mid = (prevY + nextY) / 2;
      if (Math.abs(mid - stringY) > STRING_HIT_TOLERANCE * 2.2) {
        return;
      }
      candidates.push(index);
    });

    if (candidates.length === 0) {
      // Fallback: nearest preferred string if tip is very close and moving across it.
      let best = -1;
      let bestDist = STRING_HIT_TOLERANCE;
      for (const index of preferred) {
        const stringY = GUITAR_STRING_YS[index];
        const wasAbove = prevY < stringY;
        const nowAbove = nextY < stringY;
        if (wasAbove === nowAbove) {
          continue;
        }
        const dist = Math.abs(nextY - stringY);
        if (dist < bestDist) {
          bestDist = dist;
          best = index;
        }
      }
      return best;
    }

    // Prefer the finger's natural string band when several crossed.
    const preferredHit = candidates.find((index) => preferred.includes(index));
    return preferredHit ?? candidates[0];
  }

  private releaseAllNotes(muted: boolean): void {
    for (const active of this.activeNotes.values()) {
      if (!muted) {
        toneEngine.pianoRelease(active.note);
      }
    }
    this.activeNotes.clear();
    toneEngine.panic();
  }

  private releaseMissingHands(hands: TrackedHand[]): void {
    const ids = new Set(hands.map((hand) => hand.id));
    for (const [key, active] of [...this.activeNotes.entries()]) {
      if (!ids.has(active.handId)) {
        toneEngine.pianoRelease(active.note);
        this.activeNotes.delete(key);
      }
    }
  }
}

function fingerShort(finger: FingerName): string {
  switch (finger) {
    case 'thumb':
      return 'p';
    case 'index':
      return 'i';
    case 'middle':
      return 'm';
    case 'ring':
      return 'a';
    case 'pinky':
      return 'c';
    default:
      return '?';
  }
}
