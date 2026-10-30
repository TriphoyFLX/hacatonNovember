import * as Tone from 'tone';
import type { PianoPreset } from '../types';
import type { GuitarTone } from '../instruments/guitarVoicings';
import { guitarEngine } from './guitarEngine';

class ToneEngine {
  private ready = false;
  private unlocking: Promise<void> | null = null;
  private master: Tone.Gain | null = null;
  private limiter: Tone.Limiter | null = null;
  private meter: Tone.Meter | null = null;
  private piano: Tone.PolySynth | null = null;
  private kick: Tone.MembraneSynth | null = null;
  private snare: Tone.NoiseSynth | null = null;
  private hat: Tone.MetalSynth | null = null;
  private tom: Tone.MembraneSynth | null = null;
  private crash: Tone.MetalSynth | null = null;
  private fallbackGuitar: Tone.PolySynth | null = null;
  private volume = 0.85;
  private muted = false;

  async unlock(playClick = true): Promise<void> {
    if (this.ready) {
      await Tone.start();
      if (playClick) {
        this.playTestBlip();
      }
      return;
    }

    if (this.unlocking) {
      return this.unlocking;
    }

    this.unlocking = (async () => {
      await Tone.start();
      this.buildGraph();
      guitarEngine.connect(this.master!);
      try {
        await guitarEngine.init();
      } catch {
        this.fallbackGuitar = new Tone.PolySynth(Tone.Synth, {
          oscillator: { type: 'sawtooth' },
          envelope: { attack: 0.005, decay: 0.25, sustain: 0.1, release: 0.35 },
        }).connect(this.master!);
        this.fallbackGuitar.maxPolyphony = 6;
      }
      this.ready = true;
      this.setVolume(this.volume);
      this.setMuted(this.muted);
      if (playClick) {
        this.playTestBlip();
      }
    })().finally(() => {
      this.unlocking = null;
    });

    return this.unlocking;
  }

  isReady(): boolean {
    return this.ready;
  }

  getPeak(): number {
    if (!this.meter) {
      return 0;
    }
    const value = this.meter.getValue();
    const numeric = Array.isArray(value) ? value[0] : value;
    if (!Number.isFinite(numeric)) {
      return 0;
    }
    return Math.max(0, Math.min(1, (numeric + 60) / 60));
  }

  setVolume(value: number): void {
    this.volume = Math.max(0, Math.min(1, value));
    if (this.master) {
      this.master.gain.value = this.muted ? 0 : this.volume;
    }
  }

  setMuted(muted: boolean): void {
    this.muted = muted;
    this.setVolume(this.volume);
  }

  panic(): void {
    this.piano?.releaseAll();
    this.fallbackGuitar?.releaseAll();
    guitarEngine.panic();
  }

  setPianoPreset(preset: PianoPreset): void {
    if (!this.piano) {
      return;
    }

    if (preset === 'soft-pad') {
      this.piano.set({
        oscillator: { type: 'sine' },
        envelope: { attack: 0.05, decay: 0.3, sustain: 0.55, release: 1.1 },
      });
    } else if (preset === 'bright-keys') {
      this.piano.set({
        oscillator: { type: 'triangle8' },
        envelope: { attack: 0.01, decay: 0.2, sustain: 0.25, release: 0.45 },
      });
    } else {
      this.piano.set({
        oscillator: { type: 'square' },
        envelope: { attack: 0.001, decay: 0.18, sustain: 0.05, release: 0.2 },
      });
    }
  }

  setGuitarTone(tone: GuitarTone): void {
    guitarEngine.setTone(tone);
  }

  pianoAttack(note: string, velocity: number): void {
    if (!this.ready || this.muted || !this.piano) {
      return;
    }
    this.piano.triggerAttack(note, Tone.now(), Math.max(0.15, Math.min(1, velocity)));
  }

  pianoRelease(note: string): void {
    this.piano?.triggerRelease(note, Tone.now());
  }

  playDrum(pad: 'kick' | 'snare' | 'hat' | 'tom' | 'crash', velocity: number): void {
    if (!this.ready || this.muted) {
      return;
    }
    const v = Math.max(0.2, Math.min(1, velocity));
    const now = Tone.now();

    switch (pad) {
      case 'kick':
        this.kick?.triggerAttackRelease('C1', '8n', now, v);
        break;
      case 'snare':
        this.snare?.triggerAttackRelease('16n', now, v);
        break;
      case 'hat':
        this.hat?.triggerAttackRelease('32n', now, v * 0.7);
        break;
      case 'tom':
        this.tom?.triggerAttackRelease('G2', '8n', now, v);
        break;
      case 'crash':
        this.crash?.triggerAttackRelease('8n', now, v * 0.85);
        break;
      default:
        break;
    }
  }

  strumGuitar(options: {
    notes: Array<string | null>;
    direction: 'down' | 'up';
    velocity: number;
    muted: boolean;
    tone: GuitarTone;
  }): void {
    if (!this.ready || this.muted) {
      return;
    }

    if (guitarEngine.isReady()) {
      guitarEngine.strum(options);
      return;
    }

    const notes = options.notes.filter((note): note is string => Boolean(note));
    const now = Tone.now();
    notes.forEach((note, index) => {
      this.fallbackGuitar?.triggerAttackRelease(
        note,
        '8n',
        now + index * 0.015,
        Math.max(0.2, options.velocity),
      );
    });
  }

  pluckGuitar(options: {
    note: string;
    velocity: number;
    stringIndex: number;
    muted: boolean;
    tone: GuitarTone;
  }): void {
    if (!this.ready || this.muted) {
      return;
    }

    if (guitarEngine.isReady()) {
      guitarEngine.pluck(options);
      return;
    }

    this.fallbackGuitar?.triggerAttackRelease(
      options.note,
      '8n',
      Tone.now(),
      Math.max(0.2, options.velocity),
    );
  }

  dispose(): void {
    this.panic();
    this.piano?.dispose();
    this.kick?.dispose();
    this.snare?.dispose();
    this.hat?.dispose();
    this.tom?.dispose();
    this.crash?.dispose();
    this.fallbackGuitar?.dispose();
    guitarEngine.dispose();
    this.meter?.dispose();
    this.limiter?.dispose();
    this.master?.dispose();
    this.piano = null;
    this.kick = null;
    this.snare = null;
    this.hat = null;
    this.tom = null;
    this.crash = null;
    this.fallbackGuitar = null;
    this.meter = null;
    this.limiter = null;
    this.master = null;
    this.ready = false;
  }

  private playTestBlip(): void {
    if (this.muted || !this.piano) {
      return;
    }
    const now = Tone.now();
    this.piano.triggerAttackRelease('C5', '16n', now, 0.45);
    this.piano.triggerAttackRelease('E5', '16n', now + 0.08, 0.35);
  }

  private buildGraph(): void {
    this.master = new Tone.Gain(this.volume);
    this.limiter = new Tone.Limiter(-2);
    this.meter = new Tone.Meter();
    this.master.connect(this.limiter);
    this.limiter.connect(this.meter);
    this.meter.toDestination();

    this.piano = new Tone.PolySynth(Tone.Synth, {
      oscillator: { type: 'triangle8' },
      envelope: { attack: 0.01, decay: 0.2, sustain: 0.25, release: 0.45 },
    }).connect(this.master);
    this.piano.maxPolyphony = 6;

    this.kick = new Tone.MembraneSynth({
      pitchDecay: 0.05,
      octaves: 5,
      oscillator: { type: 'sine' },
      envelope: { attack: 0.001, decay: 0.35, sustain: 0.01, release: 0.25 },
    }).connect(this.master);

    this.snare = new Tone.NoiseSynth({
      noise: { type: 'white' },
      envelope: { attack: 0.001, decay: 0.18, sustain: 0 },
    }).connect(this.master);

    this.hat = new Tone.MetalSynth({
      envelope: { attack: 0.001, decay: 0.08, release: 0.02 },
      harmonicity: 5.1,
      modulationIndex: 32,
      resonance: 4000,
      octaves: 1.5,
    }).connect(this.master);

    this.tom = new Tone.MembraneSynth({
      pitchDecay: 0.08,
      octaves: 3,
      envelope: { attack: 0.001, decay: 0.25, sustain: 0.01, release: 0.2 },
    }).connect(this.master);

    this.crash = new Tone.MetalSynth({
      envelope: { attack: 0.001, decay: 1.1, release: 0.2 },
      harmonicity: 5.1,
      modulationIndex: 22,
      resonance: 1200,
      octaves: 1.2,
    }).connect(this.master);
  }
}

export const toneEngine = new ToneEngine();
