import { describe, expect, it } from 'vitest';
import { buildConstraints, classifyCameraError, pickDevice, videoInputs } from './capture';

describe('captura de video', () => {
  it('clasifica los errores de getUserMedia', () => {
    expect(classifyCameraError({ name: 'NotAllowedError' })).toBe('denied');
    expect(classifyCameraError(new DOMException('x', 'NotAllowedError'))).toBe('denied');
    expect(classifyCameraError({ name: 'NotFoundError' })).toBe('none');
    expect(classifyCameraError({ name: 'NotReadableError' })).toBe('busy');
    expect(classifyCameraError({ name: 'TypeError' })).toBe('unsupported');
    expect(classifyCameraError({ name: 'OverconstrainedError' })).toBe('unknown');
    expect(classifyCameraError('boom')).toBe('unknown');
    expect(classifyCameraError(null)).toBe('unknown');
  });

  it('construye restricciones: cámara exacta y tamaño ideal', () => {
    expect(buildConstraints({})).toEqual({ audio: false, video: true });
    expect(buildConstraints({ deviceId: 'abc', width: 1280, height: 720 })).toEqual({
      audio: false,
      video: { deviceId: { exact: 'abc' }, width: { ideal: 1280 }, height: { ideal: 720 } },
    });
    expect(buildConstraints({ width: 640, height: null })).toEqual({ audio: false, video: true });
  });

  it('lista solo cámaras y numera las que no traen nombre (sin permiso)', () => {
    const list = videoInputs([
      { kind: 'audioinput', deviceId: 'm', label: 'Micrófono' },
      { kind: 'videoinput', deviceId: '', label: '' },
      { kind: 'videoinput', deviceId: 'b', label: 'USB 2.0' },
    ]);
    expect(list).toEqual([{ id: '', label: '#1' }, { id: 'b', label: 'USB 2.0' }]);
  });

  it('recuerda la cámara por id y, si cambió, por nombre', () => {
    const devs = [{ id: 'a', label: 'FaceTime' }, { id: 'b', label: 'USB 2.0' }];
    expect(pickDevice(devs, { id: 'b' })?.label).toBe('USB 2.0');
    expect(pickDevice(devs, { id: 'zzz', label: 'FaceTime' })?.id).toBe('a');
    expect(pickDevice(devs, { id: null, label: null })).toBeNull();
  });
});
