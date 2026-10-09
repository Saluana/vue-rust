import { parse as sfcParse } from '@vue/compiler-sfc';
import { baseParse } from '@vue/compiler-dom';
import { parse, parseExpression } from '@babel/parser';

// Deliberately a checked subset, not a general JavaScript-to-Rust translator.
const rustString = value => '"' + [...value].map(c => c === '"' ? String.fromCharCode(92,34) : c === String.fromCharCode(92) ? String.fromCharCode(92,92) : c.codePointAt(0) < 32 ? String.fromCharCode(92) + `u{${c.codePointAt(0).toString(16)}}` : c).join('') + '"';

export function compile(source, filename = 'App.vue') {
  const fail = (message) => { throw new Error(`${filename}: ${message}`); };
  const { descriptor: d, errors } = sfcParse(source, { filename });
  if (errors.length) fail(String(errors[0]));
  if (!d.scriptSetup || d.script || !d.template) fail('Use one <script setup> and one <template>; Options API is unsupported');
  if (d.scriptSetup.src || d.template.src || d.template.lang) fail('External blocks and template preprocessors are unsupported');
  if (d.styles.length || d.customBlocks.length) fail('CSS and custom blocks are not implemented yet; they are never silently dropped');
  if (d.scriptSetup.lang && d.scriptSetup.lang !== 'ts') fail('Only JavaScript and TypeScript setup scripts are supported');
  const ast = parse(d.scriptSetup.content, { sourceType: 'module', plugins: ['typescript'] });
  const symbols = new Map(), methods = new Map(), imports = new Set();
  const ident = n => {
    if (!/^[a-zA-Z][a-zA-Z0-9_]*$/.test(n) || ['self','Self','type','match','ref','loop','fn','mod','move','use','crate','super','struct','enum','impl','trait','where','async','await','dyn','pub','const','let','mut','return','break','continue','in','as','true','false','default','draw','ui'].includes(n)) fail(`Unsupported/reserved binding: ${n}`);
    return n;
  };
  for (const n of ast.program.body) {
    if (n.type === 'ImportDeclaration') {
      if (n.source.value !== 'vue') fail('Only ref and computed imports from vue are supported');
      for (const s of n.specifiers) {
        if (s.type !== 'ImportSpecifier' || !['ref','computed'].includes(s.imported.name) || s.local.name !== s.imported.name) fail('Only unaliased ref/computed imports are supported');
        imports.add(s.local.name);
      }
    } else if (n.type === 'VariableDeclaration') {
      if (n.kind !== 'const') fail('Setup bindings must use const');
      for (const v of n.declarations) {
        if (v.id.type !== 'Identifier' || v.init?.type !== 'CallExpression' || v.init.callee.type !== 'Identifier' || !['ref','computed'].includes(v.init.callee.name) || v.init.arguments.length !== 1) fail('Use const name = ref(value) or computed(() => expression)');
        if (v.id.typeAnnotation || v.init.typeParameters || v.init.typeArguments) fail('Explicit TypeScript types and generics are not implemented yet');
        const name = ident(v.id.name), kind = v.init.callee.name;
        if (symbols.has(name) || methods.has(name)) fail(`Duplicate binding ${name}`);
        const arg = v.init.arguments[0];
        if (kind === 'computed' && (arg.type !== 'ArrowFunctionExpression' || arg.params.length || arg.async || arg.body.type === 'BlockStatement')) fail('Computed requires a synchronous expression arrow');
        symbols.set(name, {kind, node: kind === 'computed' ? arg.body : arg});
      }
    } else if (n.type === 'FunctionDeclaration') {
      if (!n.id || n.params.length || n.async || n.generator) fail('Handlers must be synchronous zero-argument functions');
      const name = ident(n.id.name);
      if (methods.has(name) || symbols.has(name)) fail(`Duplicate binding ${name}`);
      methods.set(name, n.body.body);
    } else fail(`Unsupported setup syntax: ${n.type}`);
  }
  for (const s of symbols.values()) if (!imports.has(s.kind)) fail(`Import ${s.kind} from vue`);
  const resolving = new Set();
  const rustType = t => ({number:'f64',string:'String',boolean:'bool'}[t]);
  const lit = n => {
    if (n.type === 'NumericLiteral' && Number.isFinite(n.value)) return {code:`${n.value.toFixed(1) === String(n.value)+'.0' ? n.value+'.0' : n.value}f64`,type:'number'};
    if (n.type === 'StringLiteral') return {code:`String::from(${rustString(n.value)})`,type:'string'};
    if (n.type === 'BooleanLiteral') return {code:String(n.value),type:'boolean'};
    return null;
  };
  const read = name => {
    const s = symbols.get(name); if (!s) fail(`Unknown binding ${name}`);
    if (!s.type) {
      if (resolving.has(name)) fail('Cyclic computed dependencies'); resolving.add(name);
      const e = expr(s.node, false); s.type=e.type; s.initial=e.code; resolving.delete(name);
      if (s.kind === 'ref' && !lit(s.node) && !(s.node.type === 'UnaryExpression' && s.node.operator === '-' && lit(s.node.argument)?.type === 'number')) fail('Ref initializers must be primitive literals');
    }
    return {code:s.kind === 'computed' ? `self.${name}()` : `self.${name}${s.type === 'string' ? '.clone()' : ''}`,type:s.type};
  };
  const expr = (n, template=false) => {
    const l=lit(n); if(l) return l;
    if(n.type==='Identifier' && template) return read(n.name);
    if(n.type==='MemberExpression' && !n.computed && n.object.type==='Identifier' && n.property.name==='value') return read(n.object.name);
    if(n.type==='UnaryExpression' && ['-','!'].includes(n.operator)) {
      const a=expr(n.argument,template); if(a.type !== (n.operator==='!'?'boolean':'number')) fail('Unary operand type mismatch');
      return {code:`(${n.operator}${a.code})`,type:a.type};
    }
    if(n.type==='BinaryExpression' || n.type==='LogicalExpression') {
      const a=expr(n.left,template), b=expr(n.right,template), op=n.operator;
      if(op==='+' && (a.type==='string'||b.type==='string')) return {code:`format!("{}{}", ${a.code}, ${b.code})`,type:'string'};
      if(['+','-','*','/','%','<','>','<=','>='].includes(op)) {
        if(a.type!=='number'||b.type!=='number') fail('Numeric operands required');
        return {code:`(${a.code} ${op} ${b.code})`,type:['<','>','<=','>='].includes(op)?'boolean':'number'};
      }
      if(['===','!=='].includes(op)) {if(a.type!==b.type) fail('Equality operand type mismatch');return {code:`(${a.code} ${op==='==='?'==':'!='} ${b.code})`,type:'boolean'};}
      if(['&&','||'].includes(op) && a.type==='boolean' && b.type==='boolean') return {code:`(${a.code} ${op} ${b.code})`,type:'boolean'};
    }
    if(n.type==='ConditionalExpression') {
      const a=expr(n.test,template),b=expr(n.consequent,template),c=expr(n.alternate,template);
      if(a.type!=='boolean'||b.type!==c.type) fail('Conditional type mismatch');
      return {code:`(if ${a.code} { ${b.code} } else { ${c.code} })`,type:b.type};
    }
    fail(`Unsupported expression: ${n.type}${n.operator?' '+n.operator:''}`);
  };
  for (const name of symbols.keys()) read(name);
  const target = (n, template) => {
    const name = n.type==='Identifier' && template ? n.name : n.type==='MemberExpression' && !n.computed && n.object.type==='Identifier' && n.property.name==='value' ? n.object.name : null;
    if(!name || symbols.get(name)?.kind!=='ref') fail('Only refs can be mutated'); return {name,type:symbols.get(name).type};
  };
  const action=(n,template=false)=>{
    if(n.type==='Identifier' && template && methods.has(n.name)) return `self.${n.name}();`;
    if(n.type==='UpdateExpression') {const t=target(n.argument,template);if(t.type!=='number') fail('Increment needs number ref');return `self.${t.name} ${n.operator==='++'?'+=':'-='} 1.0;`;}
    if(n.type==='AssignmentExpression') {const t=target(n.left,template),r=expr(n.right,template);if(t.type!==r.type || !['=','+=','-='].includes(n.operator) || (n.operator!=='=' && t.type!=='number')) fail('Assignment type/operator mismatch');return `self.${t.name} ${n.operator} ${r.code};`;}
    if(n.type==='CallExpression' && n.callee.type==='Identifier' && methods.has(n.callee.name) && n.arguments.length===0) return `self.${n.callee.name}();`;
    fail(`Unsupported handler: ${n.type}`);
  };
  const methodCode=[...methods].map(([name,body])=>`pub fn ${name}(&mut self) { ${body.map(n=>n.type==='ExpressionStatement'?action(n.expression):fail('Handlers currently accept mutations and handler calls only')).join('\n')} }`).join('\n');
  const expression=s=>parseExpression(s,{plugins:['typescript']});
  const template=baseParse(d.template.content);
  const text=children=>{
    const chunks=children.map(n=>n.type===2?`String::from(${rustString(n.content)})`:n.type===5?`format!("{}", ${expr(expression(n.content.content),true).code})`:fail('This control accepts only text and interpolations'));
    return chunks.length?`[${chunks.join(',')}].concat()`:'String::new()';
  };
  let inputId=0;
  const render=n=>{
    if(n.type===3) return '';
    if(n.type===2) return n.content.trim()?`ui.label(${text([n])});`:'';
    if(n.type===5) return `ui.label(${text([n])});`;
    if(n.type!==1) fail('Unsupported template node');
    const directives=new Map();
    for(const p of n.props){
      if(p.type!==7) fail(`Static attribute ${p.name} is not supported yet`);
      const key=p.name+(p.arg?':'+p.arg.content:'');
      if(p.modifiers.length || !['if','on:click','bind:disabled','model'].includes(key) || directives.has(key) || !p.exp) fail(`Unsupported/duplicate directive ${key}`);
      directives.set(key,p.exp.content);
    }
    for(const key of directives.keys()) if((key==='on:click'||key==='bind:disabled')&&n.tag!=='button'||key==='model'&&n.tag!=='input') fail(`Directive ${key} is invalid on ${n.tag}`);
    let code;
    if(['main','section','div'].includes(n.tag)) code=`ui.vertical(|ui| {${n.children.map(render).join('\n')}});`;
    else if(['h1','h2'].includes(n.tag)) code=`ui.heading(${text(n.children)});`;
    else if(['p','span','label'].includes(n.tag)) code=`ui.label(${text(n.children)});`;
    else if(n.tag==='button') {
      const disabled=directives.get('bind:disabled'); const de=disabled?expr(expression(disabled),true):{code:'false',type:'boolean'};
      if(de.type!=='boolean') fail('disabled must be boolean');
      const click=directives.get('on:click');
      code=`if ui.add_enabled(!(${de.code}), egui::Button::new(${text(n.children)})).clicked() { ${click?action(expression(click),true):''} }`;
    } else if(n.tag==='input') {
      const model=directives.get('model');if(!model)fail('Input requires v-model');
      const m=expression(model),t=target(m,true);if(t.type!=='string')fail('Input currently requires a string ref');
      code=`ui.push_id(${inputId++}, |ui| { ui.text_edit_singleline(&mut self.${t.name}); });`;
    } else fail(`Unsupported element <${n.tag}>`);
    if(directives.has('if')) {const condition=expr(expression(directives.get('if')),true);if(condition.type!=='boolean')fail('v-if requires boolean');code=`if ${condition.code} {${code}}`;}
    return code;
  };
  const rendering=template.children.map(render).join('\n');
  const refs=[...symbols].filter(([,s])=>s.kind==='ref');
  return `// Generated by vue-rust-lab. Edit the .vue source, not this file.\n#![allow(unused_parens)]\nuse eframe::egui;\n#[derive(Clone, Debug)]\npub struct App { ${refs.map(([n,s])=>`pub ${n}: ${rustType(s.type)},`).join('\n')} }\nimpl Default for App { fn default() -> Self { Self {${refs.map(([n,s])=>`${n}: ${s.initial},`).join('\n')}} } }\nimpl App {\n${[...symbols].filter(([,s])=>s.kind==='computed').map(([n,s])=>`pub fn ${n}(&self) -> ${rustType(s.type)} { ${s.initial} }`).join('\n')}\n${methodCode}\npub fn draw(&mut self, ui: &mut egui::Ui) { ${rendering} }\n}\nimpl eframe::App for App { fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) { self.draw(ui); } }\n`;
}
