import * as Tone from 'tone';
import type { GuitarTone } from '../instruments/guitarVoicings';

export interface GuitarStrumOptions {
  notes: Array<string | null>;
  direction: 'down' | 'up';
  velocity: number;
  muted: boolean;
  tone: GuitarTone;
}

export interface GuitarPluckOptions {
  note: string;
  velocity: number;
  stringIndex: number;
  muted: boolean;
  tone: GuitarTone;
}

type Register = 'bass' | 'mid' | 'high';

/** One-shot sample is C major pluck rooted at C3. */
const SAMPLE_URL = '/audio/guitar-pluck-c.wav';
const SAMPLE_ROOT = 'C3';

function registerForString(stringIndex: number): Register {
  if (stringIndex <= 1) {
    return 'bass';
  }
  if (stringIndex <= 3) {
    return 'mid';
  }
  return 'high';
}

interface RegisterBus {
  sampler: Tone.Sampler;
  body: Tone.Synth | null;
  input: Tone.Gain;
  eqLow: Tone.Filter;
  eqHigh: Tone.Filter;
  presence: Tone.Filter;
  drive: Tone.Distortion;
  room: Tone.Freeverb;
  out: Tone.Gain;
}

/**
 * Maxed guitar: one pluck sample routed through bass / mid / high buses
 * with body layer on low strings and register-tuned dynamics.
 */
export class GuitarEngine {
  private ready = false;
  private loading: Promise<void> | null = null;
  private buses: Partial<Record<Register, RegisterBus>> = {};
  private compressor: Tone.Compressor | null = null;
  private master: Tone.Gain | null = null;
  private lastFire: number[] = [];
  private destination: Tone.ToneAudioNode | null = null;

  connect(destination: Tone.ToneAudioNode): void {
    this.destination = destination;
  }

  async init(): Promise<void> {
    if (this.ready) {
      return;
    }
    if (this.loading) {
      return this.loading;
    }
    if (!this.destination) {
      throw new Error('GuitarEngine destination is not connected');
    }

    this.loading = (async () => {
      this.master = new Tone.Gain(1.15);
      this.compressor = new Tone.Compressor({
        threshold: -18,
        ratio: 3.2,
        attack: 0.01,
        release: 0.18,
      });
      this.master.connect(this.compressor);
      this.compressor.connect(this.destination!);

      this.buses.bass = this.buildBus('bass');
      this.buses.mid = this.buildBus('mid');
      this.buses.high = this.buildBus('high');

      await Tone.loaded();

      this.lastFire = Array.from({ length: 6 }, () => 0);
      this.applyTone('clean');
      this.ready = true;
    })().finally(() => {
      this.loading = null;
    });

    return this.loading;
  }

  isReady(): boolean {
    return this.ready;
  }

  setTone(tone: GuitarTone): void {
    this.applyTone(tone);
  }

  strum(options: GuitarStrumOptions): void {
    if (!this.ready) {
      return;
    }

    this.applyTone(options.tone);
    const now = Tone.now();
    const velocity = Math.max(0.28, Math.min(1, options.velocity));
    const ordered = options.notes
      .map((note, index) => ({ note, index }))
      .filter((item): item is { note: string; index: number } => Boolean(item.note));

    const sequence = options.direction === 'down' ? ordered : [...ordered].reverse();
    // Realistic rake: bass lags a bit more on downstrokes.
    const spread = options.muted ? 0.009 : options.direction === 'down' ? 0.022 : 0.016;

    sequence.forEach((item, order) => {
      const stringBoost = item.index <= 1 ? 1.12 : item.index >= 4 ? 1.08 : 1;
      this.fireNote(
        item.index,
        item.note,
        velocity * stringBoost * (0.78 + order * 0.035),
        now + order * spread,
        options.muted,
      );
    });
  }

  pluck(options: GuitarPluckOptions): void {
    if (!this.ready) {
      return;
    }
    this.applyTone(options.tone);
    const boost =
      options.stringIndex <= 1 ? 1.15 : options.stringIndex >= 4 ? 1.1 : 1.05;
    this.fireNote(
      options.stringIndex,
      options.note,
      Math.max(0.28, Math.min(1, options.velocity * boost)),
      Tone.now(),
      options.muted,
    );
  }

  panic(): void {
    (['bass', 'mid', 'high'] as Register[]).forEach((register) => {
      const bus = this.buses[register];
      bus?.sampler.releaseAll();
      bus?.body?.triggerRelease();
    });
    if (this.master) {
      const now = Tone.now();
      this.master.gain.cancelScheduledValues(now);
      this.master.gain.setValueAtTime(0, now);
      this.master.gain.linearRampToValueAtTime(1.15, now + 0.06);
    }
  }

  dispose(): void {
    (['bass', 'mid', 'high'] as Register[]).forEach((register) => {
      const bus = this.buses[register];
      if (!bus) {
        return;
      }
      bus.sampler.dispose();
      bus.body?.dispose();
      bus.room.dispose();
      bus.drive.dispose();
      bus.presence.dispose();
      bus.eqHigh.dispose();
      bus.eqLow.dispose();
      bus.input.dispose();
      bus.out.dispose();
    });
    this.buses = {};
    this.compressor?.dispose();
    this.master?.dispose();
    this.compressor = null;
    this.master = null;
    this.lastFire = [];
    this.ready = false;
  }

  private buildBus(register: Register): RegisterBus {
    const input = new Tone.Gain(1);
    const eqLow = new Tone.Filter({ type: 'lowshelf', frequency: 180, Q: 0.7 });
    const eqHigh = new Tone.Filter({ type: 'highshelf', frequency: 3200, Q: 0.7 });
    const presence = new Tone.Filter({ type: 'peaking', frequency: 1200, Q: 0.9 });
    const drive = new Tone.Distortion(0.18);
    drive.wet.value = 0.12;
    const room =
      register === 'bass'
        ? new Tone.Freeverb({ roomSize: 0.38, dampening: 2200, wet: 0.2 })
        : register === 'mid'
          ? new Tone.Freeverb({ roomSize: 0.3, dampening: 2800, wet: 0.16 })
          : new Tone.Freeverb({ roomSize: 0.24, dampening: 4200, wet: 0.12 });
    const out = new Tone.Gain(1);

    if (register === 'bass') {
      eqLow.frequency.value = 140;
      eqLow.gain.value = 7;
      eqHigh.frequency.value = 2200;
      eqHigh.gain.value = -3;
      presence.frequency.value = 220;
      presence.gain.value = 4;
      out.gain.value = 1.25;
    } else if (register === 'mid') {
      eqLow.frequency.value = 200;
      eqLow.gain.value = 1.5;
      eqHigh.frequency.value = 3500;
      eqHigh.gain.value = 2;
      presence.frequency.value = 1100;
      presence.gain.value = 5;
      out.gain.value = 1.12;
    } else {
      eqLow.frequency.value = 260;
      eqLow.gain.value = -2;
      eqHigh.frequency.value = 4800;
      eqHigh.gain.value = 6;
      presence.frequency.value = 2800;
      presence.gain.value = 4.5;
      out.gain.value = 1.18;
    }

    input.connect(eqLow);
    eqLow.connect(eqHigh);
    eqHigh.connect(presence);
    presence.connect(drive);
    drive.connect(room);
    room.connect(out);
    out.connect(this.master!);

    const sampler = new Tone.Sampler({
      urls: { [SAMPLE_ROOT]: SAMPLE_URL },
      attack: 0,
      release: register === 'bass' ? 2.2 : register === 'mid' ? 1.5 : 1.1,
    });
    sampler.connect(input);

    let body: Tone.Synth | null = null;
    if (register === 'bass') {
      body = new Tone.Synth({
        oscillator: { type: 'triangle' },
        envelope: { attack: 0.01, decay: 0.45, sustain: 0.15, release: 0.7 },
      });
      const bodyGain = new Tone.Gain(0.28);
      body.connect(bodyGain);
      bodyGain.connect(input);
    } else if (register === 'high') {
      // Air / pick sparkle layer for top strings.
      body = new Tone.Synth({
        oscillator: { type: 'square8' },
        envelope: { attack: 0.001, decay: 0.12, sustain: 0.02, release: 0.18 },
      });
      const sparkle = new Tone.Gain(0.08);
      body.connect(sparkle);
      sparkle.connect(input);
    }

    return { sampler, body, input, eqLow, eqHigh, presence, drive, room, out };
  }

  private fireNote(
    stringIndex: number,
    note: string,
    velocity: number,
    time: number,
    muted: boolean,
  ): void {
    const register = registerForString(stringIndex);
    const bus = this.buses[register];
    if (!bus) {
      return;
    }

    const safeTime = Math.max(time, (this.lastFire[stringIndex] ?? 0) + 0.018, Tone.now());
    this.lastFire[stringIndex] = safeTime;

    const duration = muted
      ? 0.07
      : register === 'bass'
        ? 2.4
        : register === 'mid'
          ? 1.7
          : 1.15;
    const amp = muted
      ? velocity * 0.32
      : velocity * (register === 'bass' ? 1.05 : register === 'high' ? 0.98 : 1);

    try {
      bus.sampler.triggerAttackRelease(note, duration, safeTime, amp);
      if (bus.body && !muted) {
        if (register === 'bass') {
          bus.body.triggerAttackRelease(note, duration * 0.55, safeTime, amp * 0.55);
        } else if (register === 'high') {
          bus.body.triggerAttackRelease(note, 0.08 + velocity * 0.06, safeTime, amp * 0.35);
        }
      }
    } catch {
      // Ignore rare schedule races under high gesture rate.
    }
  }

  private applyTone(tone: GuitarTone): void {
    (['bass', 'mid', 'high'] as Register[]).forEach((register) => {
      const bus = this.buses[register];
      if (!bus) {
        return;
      }

      if (tone === 'clean') {
        bus.drive.wet.value = register === 'bass' ? 0.06 : 0.04;
        bus.drive.distortion = 0.08;
        bus.room.wet.value = register === 'bass' ? 0.22 : register === 'high' ? 0.14 : 0.18;
        bus.out.gain.value = register === 'bass' ? 1.28 : register === 'high' ? 1.2 : 1.14;
      } else if (tone === 'crunch') {
        bus.drive.wet.value = register === 'bass' ? 0.42 : 0.5;
        bus.drive.distortion = register === 'bass' ? 0.32 : 0.45;
        bus.room.wet.value = 0.12;
        bus.out.gain.value = register === 'bass' ? 1.18 : 1.05;
      } else {
        bus.drive.wet.value = 0.72;
        bus.drive.distortion = register === 'high' ? 0.7 : 0.58;
        bus.room.wet.value = register === 'high' ? 0.24 : 0.16;
        bus.out.gain.value = register === 'high' ? 1.22 : 1.0;
      }
    });

    if (this.master) {
      this.master.gain.value = tone === 'lead' ? 1.05 : 1.15;
    }
  }
}

export const guitarEngine = new GuitarEngine();
