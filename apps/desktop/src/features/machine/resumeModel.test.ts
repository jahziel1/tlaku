import { describe, expect, it } from 'vitest';
import type { ResumeReadiness, ResumeView } from '@/api/types';
import { choiceAt, homeMode, nearestMove, resumeBlockedReason, suggestedChoice, type ResumeGate } from './resumeModel';

const view = {
  moves: [0, 0, 0, 0, 1, 10, 0, 50, 0, 10, 10, 0, 1, 0, 10, 50],
  move_lines: [3, 4, 5, 6],
  bounds: [
    [0, 0],
    [10, 10],
  ],
  suggested: { line: 5, move_index: 2, at: [10, 4], executing_line: 5, distance_mm: 0, mismatch: false, row_restart: false, percent: 40, z_delta: 0 },
} as unknown as ResumeView;

const ready = (homing: boolean | null, homed: boolean): ResumeReadiness => ({ homing, homed, state: 'idle' });
const ok: ResumeGate = { connected: true, sameMachine: true, jobBusy: false, state: 'idle', ready: ready(true, true), hasPoint: true, unmoved: false, framedRun: true, framedOk: true, checklistOk: true };

describe('restart choice', () => {
  it('uses the engine suggestion as is, and a hand-picked move always starts at the line start', () => {
    expect(suggestedChoice(view)).toEqual({ line: 5, at: [10, 4], moveIndex: 2, percent: 40, custom: false });
    const c = choiceAt(view, 3);
    expect(c).toMatchObject({ line: 6, at: null, moveIndex: 3, custom: true });
    expect(choiceAt(view, 99).moveIndex).toBe(3);
    expect(choiceAt(view, -4).moveIndex).toBe(1);
  });

  it('finds the move nearest to a point', () => {
    expect(nearestMove(view, 9, 1)).toBe(1);
    expect(nearestMove(view, 1, 9)).toBe(3);
  });
});

describe('resumeBlockedReason', () => {
  it('lets a fully prepared homing machine continue', () => {
    expect(resumeBlockedReason(ok)).toBeNull();
  });

  it('asks for everything in order', () => {
    expect(resumeBlockedReason({ ...ok, connected: false })).toBe('machine.resume.blocked.connect');
    expect(resumeBlockedReason({ ...ok, sameMachine: false })).toBe('machine.resume.blocked.wrong');
    expect(resumeBlockedReason({ ...ok, state: 'alarm' })).toBe('machine.resume.blocked.busy');
    expect(resumeBlockedReason({ ...ok, jobBusy: true })).toBe('machine.resume.blocked.busy');
    expect(resumeBlockedReason({ ...ok, ready: null })).toBe('machine.resume.blocked.checking');
    expect(resumeBlockedReason({ ...ok, ready: ready(true, false) })).toBe('machine.resume.blocked.home');
    expect(resumeBlockedReason({ ...ok, framedRun: false })).toBe('machine.resume.blocked.frame');
    expect(resumeBlockedReason({ ...ok, framedOk: false })).toBe('machine.resume.blocked.frameOk');
    expect(resumeBlockedReason({ ...ok, checklistOk: false })).toBe('machine.resume.blocked.checklist');
  });

  it('without homing the confirmation replaces the home, never the frame', () => {
    const nohome = { ...ok, ready: ready(false, false) };
    expect(homeMode(nohome.ready)).toBe('confirm');
    expect(resumeBlockedReason(nohome)).toBe('machine.resume.blocked.confirm');
    expect(resumeBlockedReason({ ...nohome, unmoved: true })).toBeNull();
    expect(resumeBlockedReason({ ...nohome, unmoved: true, framedRun: false })).toBe('machine.resume.blocked.frame');
    // Marlin/Smoothie no contestan `$$`: sin dato se pide la confirmación, no se supone homing.
    expect(homeMode(ready(null, false))).toBe('confirm');
  });
});
