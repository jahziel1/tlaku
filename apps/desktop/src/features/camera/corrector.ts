import { FRAGMENT_SHADER, VERTEX_SHADER, lensUniforms, type LensParams } from './lens';

export interface CorrectorTarget {
  /** Tamaño en píxeles del lienzo de salida. */
  width: number;
  height: number;
  /** Lente a corregir; `null` = pasar el cuadro tal cual. */
  lens: LensParams | null;
  /** Pasar la imagen a escala de grises. */
  gray?: boolean;
}

export interface Corrector {
  readonly canvas: HTMLCanvasElement;
  /** `true` si usa WebGL (si no, solo copia el cuadro sin corregir). */
  readonly gpu: boolean;
  render(source: HTMLVideoElement | HTMLCanvasElement, target: CorrectorTarget): void;
  dispose(): void;
}

function compile(gl: WebGLRenderingContext, type: number, src: string): WebGLShader {
  const sh = gl.createShader(type);
  if (!sh) throw new Error('no se pudo crear el shader');
  gl.shaderSource(sh, src);
  gl.compileShader(sh);
  if (!gl.getShaderParameter(sh, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(sh) ?? 'shader inválido');
  return sh;
}

function createGl(canvas: HTMLCanvasElement): Corrector | null {
  // preserveDrawingBuffer: el lienzo del diseño copia este canvas con drawImage en otro momento.
  const gl = canvas.getContext('webgl', { alpha: true, premultipliedAlpha: true, preserveDrawingBuffer: true, antialias: false });
  if (!gl) return null;
  try {
    const prog = gl.createProgram();
    if (!prog) return null;
    gl.attachShader(prog, compile(gl, gl.VERTEX_SHADER, VERTEX_SHADER));
    gl.attachShader(prog, compile(gl, gl.FRAGMENT_SHADER, FRAGMENT_SHADER));
    gl.linkProgram(prog);
    if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) return null;
    gl.useProgram(prog);

    const buf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buf);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), gl.STATIC_DRAW);
    const pos = gl.getAttribLocation(prog, 'aPos');
    gl.enableVertexAttribArray(pos);
    gl.vertexAttribPointer(pos, 2, gl.FLOAT, false, 0, 0);

    const tex = gl.createTexture();
    gl.bindTexture(gl.TEXTURE_2D, tex);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);

    const u = (name: string) => gl.getUniformLocation(prog, name);
    const uSize = u('uSize');
    const uK = u('uK');
    const uRad = u('uRad');
    const uTan = u('uTan');
    const uZoom = u('uZoom');
    const uFisheye = u('uFisheye');
    const uGray = u('uGray');
    const uCorrect = u('uCorrect');
    gl.uniform1i(u('uTex'), 0);

    return {
      canvas,
      gpu: true,
      render(source, target) {
        if (canvas.width !== target.width || canvas.height !== target.height) {
          canvas.width = target.width;
          canvas.height = target.height;
        }
        gl.viewport(0, 0, canvas.width, canvas.height);
        gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, source);
        if (target.lens) {
          const p = lensUniforms(target.lens);
          gl.uniform2f(uSize, p.size[0], p.size[1]);
          gl.uniform4f(uK, ...p.k);
          gl.uniform4f(uRad, ...p.rad);
          gl.uniform2f(uTan, ...p.tan);
          gl.uniform1f(uZoom, p.zoom);
          gl.uniform1f(uFisheye, p.fisheye);
          gl.uniform1f(uCorrect, 1);
        } else {
          gl.uniform1f(uCorrect, 0);
        }
        gl.uniform1f(uGray, target.gray ? 1 : 0);
        gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
      },
      dispose() {
        gl.deleteTexture(tex);
        gl.deleteBuffer(buf);
        gl.deleteProgram(prog);
      },
    };
  } catch {
    return null;
  }
}

/** Corrector sin GPU: copia el cuadro. Se usa solo si el navegador no ofrece WebGL. */
function createCopy(canvas: HTMLCanvasElement): Corrector {
  const ctx = canvas.getContext('2d');
  return {
    canvas,
    gpu: false,
    render(source, target) {
      canvas.width = target.width;
      canvas.height = target.height;
      ctx?.drawImage(source, 0, 0, target.width, target.height);
      if (target.gray && ctx) {
        const img = ctx.getImageData(0, 0, canvas.width, canvas.height);
        const d = img.data;
        for (let i = 0; i < d.length; i += 4) {
          const g = 0.299 * d[i]! + 0.587 * d[i + 1]! + 0.114 * d[i + 2]!;
          d[i] = d[i + 1] = d[i + 2] = g;
        }
        ctx.putImageData(img, 0, 0);
      }
    },
    dispose() {},
  };
}

/** Crea el corrector sobre un canvas nuevo: WebGL si existe; si no, una copia sin corrección. */
export function createCorrector(): Corrector {
  return createGl(document.createElement('canvas')) ?? createCopy(document.createElement('canvas'));
}
