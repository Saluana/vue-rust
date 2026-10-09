// Style blocks (WP-F1 step 6): lang and module policy, src files, compileStyle per block with scoped and v-bind()
// handling, and the record of each block. The CSS is recorded as compileStyle produced it; it is not interpreted here.
import { compileStyle, parse } from '@vue/compiler-sfc';
import { createLineMap } from '../diagnostics/diagnostic.mjs';

// v-bind() variables of one block, found by the official parser on that block alone.
function cssVarsOf(content, filename) {
  if (content.includes('</style')) return [];
  return parse(`<style>\n${content}\n</style>`, { filename }).descriptor.cssVars;
}

// Offset in content of a postcss error (1-based line and column), or null when the error has no position.
function errorOffset(content, err) {
  if (!Number.isInteger(err.line) || !Number.isInteger(err.column)) return null;
  try {
    return createLineMap(content).offsetAt(err.line, err.column - 1);
  } catch {
    return null;
  }
}

// Compiles every style block. Returns the styles array; policy and compile errors are reported on ctx.
export function compileStyles(ctx, descriptor) {
  const styles = [];
  descriptor.styles.forEach((block, index) => {
    if (block.lang && block.lang !== 'css') {
      ctx.report('VN6004', ctx.rel, ...ctx.blockSpan(block), { message: `<style lang="${block.lang}"> is not supported` });
      return;
    }
    if (block.module) {
      ctx.report('VN6015', ctx.rel, ...ctx.blockSpan(block), { message: '<style module> is not supported by the synchronous frontend' });
      return;
    }
    let content;
    let file;
    let base;
    if (block.src) {
      const f = ctx.readBlockFile(block, block.src);
      if (!f) return;
      content = f.text;
      file = f.rel;
      base = 0;
    } else {
      content = block.content;
      file = ctx.rel;
      base = block.loc.start.offset;
    }
    const scoped = !!block.scoped;
    let res;
    try {
      res = compileStyle({
        source: content,
        filename: file,
        id: ctx.shortId,
        scoped,
        trim: true,
        isProd: true,
        inMap: block.src ? undefined : block.map,
      });
    } catch (err) {
      ctx.report('VN6014', ctx.rel, ...ctx.blockSpan(block), { message: `style compile failed: ${err.message}` });
      return;
    }
    for (const err of res.errors) {
      const off = errorOffset(content, err);
      const message = `CSS error: ${String(err.reason ?? err.message).split('\n')[0]}`;
      if (off === null) {
        const [s, e] = file === ctx.rel ? ctx.blockSpan(block) : [0, 0];
        ctx.report('VN6014', file, s, e, { message });
      } else {
        ctx.report('VN6014', file, base + off, base + off + 1, { message });
      }
    }
    if (res.errors.length) return;
    styles.push({
      code: res.code,
      scoped,
      module: null,
      scopeId: scoped ? ctx.scopeId : null,
      vars: cssVarsOf(content, file),
      map: res.map ?? null,
      filename: file,
      index,
    });
  });
  return styles;
}
