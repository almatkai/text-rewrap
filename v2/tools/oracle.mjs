// SPDX-License-Identifier: MIT
// Execute the vendored ORIGINAL TypeScript; no reference reimplementation.
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import assert from 'node:assert/strict';
import { transformSync } from 'esbuild';
const base = path.resolve(import.meta.dirname, '../fixtures/original');
function loader(mocks = {}) {
 const cache = new Map();
 function load(file) {
  file = path.resolve(file);
  if (!path.extname(file)) file += fs.existsSync(file+'.ts') ? '.ts' : '.tsx';
  if (cache.has(file)) return cache.get(file);
  const module = {exports:{}}; cache.set(file,module.exports);
  const code = transformSync(fs.readFileSync(file,'utf8'),{loader:file.endsWith('tsx')?'tsx':'ts',format:'cjs',jsx:'automatic'}).code;
  const require = name => name in mocks ? mocks[name] : name.startsWith('.') ? load(path.resolve(path.dirname(file),name)) : (()=>{throw Error('Unexpected import '+name)})();
  vm.runInNewContext(code,{module,exports:module.exports,require,console:{log(){}},Buffer},{filename:file});
  cache.set(file,module.exports); return module.exports;
 }
 return load;
}
const load=loader();
const {rewrapText,rewrapParagraphs}=load(base+'/src/utils/rewrapText.ts');
const {validateWidth,formatWrappingMessage}=load(base+'/src/utils/validation.ts');
const widths=['',' ','80','1','2','3','4','5','10','0','-1','-0','+0','abc','80.5','80junk','1e3','0x10','+0Xff','-0xf','0b11','Infinity','NaN','+','0x','\ufeff+3\u2029','\u008580','9'.repeat(400),'0x'+'f'.repeat(300),'100000000000000000000','1000000000000000000000','9007199254740993','0x20000000000001'];
let seed=0x51ace; const rand=n=>{seed=(Math.imul(seed,1664525)+1013904223)>>>0;return seed%n};
const tokens=['a','bb','cat','😀','❤️','👩‍💻','café','é','e\u0301','\u0085','\u180e','\u200b','\ufeff','\u2028','\u2029','\u00a0','\r','\n','\t',' ','\v','\f','\u1680','\u2000','\u200a','\u202f','\u205f','\u3000'];
const texts=['','   ','a\r\nb\r\n\r\nc','a\n\r\nb','a\n\n\n  b','😀 a b','❤️ x','a\u0085b\ufeffc','\n\nabc\n\n'];
for(let i=0;i<1500;i++) texts.push(Array.from({length:rand(100)},()=>tokens[rand(tokens.length)]).join(''));
for(let i=0;i<500;i++) widths.push((rand(2)?'+':'')+Array.from({length:1+rand(340)},()=>String(rand(10))).join('')+['junk','.4','e8','',''][rand(5)]);
const validation=widths.map(input=>{const r=validateWidth(input);return {input,valid:r.isValid,error:r.error??null,width:!r.isValid?null:r.value===undefined?'unwrap':String(r.value),message:r.isValid?formatWrappingMessage(r.value):null}});
const engine=texts.flatMap((text,i)=>['','1','2','3','4','5','10','80',widths[i%widths.length]].map(input=>{const r=validateWidth(input);return {text,input,output:rewrapText(text,r.isValid?r.value:80)}}));
const pure=[];
for(const n of [undefined,0,-1,NaN,Infinity,1,2,80]) for(const text of texts.slice(0,35)) pure.push({text,width:n===undefined?'undefined':String(n),output:rewrapText(text,n),paragraphs:rewrapParagraphs(text,n)});
// Component oracle runs original callbacks, preferences, and conditional JSX/actions.
const components=[];
for(const command of ['form','clipboard','selection']) for(const prefs of [{},{defaultWidth:'',defaultCopyToClipboard:false},{defaultWidth:'12',defaultCopyToClipboard:true}]) {
 let state=[],cursor=0, calls=[], clipboard='a b c', selection='a b c', copyError=false;
 const jsx=(type,props)=>({type,props:props??{}});
 const Form=Object.assign('Form',{TextArea:'Form.TextArea',TextField:'Form.TextField',Checkbox:'Form.Checkbox',Description:'Form.Description'});
 const Action=Object.assign('Action',{SubmitForm:'SubmitForm',CopyToClipboard:'Copy'});
 const l=loader({'react':{useState(init){const i=cursor++;state[i]??=init;return [state[i],value=>state[i]=value]}},'react/jsx-runtime':{jsx,jsxs:jsx},'@raycast/api':{Form,Action,ActionPanel:'ActionPanel',getPreferenceValues:()=>prefs,Clipboard:{async readText(){calls.push('read');return clipboard},async copy(value){calls.push(['copy',value]);if(copyError)throw new Error('copy unavailable')}},async getSelectedText(){calls.push('selection');if(selection instanceof Error)throw selection;return selection},showToast(t){calls.push(['toast',t]);return Promise.resolve()},Toast:{Style:{Failure:'failure',Success:'success'}}}});
 const Command=l(base+`/src/text-rewrap-${command}.tsx`).default;
 const render=()=>{cursor=0;const form=Command();const actions=form.props.actions.type(form.props.actions.props);return {form,actions}};
 let r=render(); const children=()=>[r.form.props.children].flat().filter(Boolean);
 const initial=children().map(n=>({type:n.type,...n.props}));
 assert.equal(initial.find(n=>n.id==='width').defaultValue,prefs.defaultWidth||'80');
 assert.equal(initial.find(n=>n.id==='copyResultToClipboard').defaultValue,prefs.defaultCopyToClipboard??true);
 const submit=async(values)=>{await [r.actions.props.children].flat().filter(Boolean)[0].props.onSubmit(values);r=render()};
 await submit({text:'a b c',width:'0',copyResultToClipboard:false});assert(!calls.includes('read'));assert(!calls.includes('selection'));
 const invalid=structuredClone(calls);calls=[];
 // Source logs selection; deliberately capture/suppress it, never emit private content.
 // Replace VM console for selection via loader's configurable harmless logging below.
 await submit({text:'a b c',width:'3',copyResultToClipboard:false});
 assert(!calls.some(c=>Array.isArray(c)&&c[0]==='copy'));
 assert.equal(children().find(n=>n.type==='Form.Description')?.props.text,'a b\nc');
 const actions=[r.actions.props.children].flat().filter(Boolean);
 assert.equal(actions[1].props.title,'Copy Rewrapped Text');assert.deepEqual(Array.from(actions[1].props.shortcut.modifiers),['cmd']);
 const noCopy=structuredClone(calls);calls=[];
 await submit({text:'a b c',width:'',copyResultToClipboard:true});
 assert(calls.some(c=>Array.isArray(c)&&c[0]==='copy'&&c[1]==='a b c'));
 const copied=structuredClone(calls);calls=[];
 if(command!=='form') {
  if(command==='clipboard')clipboard='';else selection='';
  await submit({text:'',width:'3',copyResultToClipboard:false});
  assert(calls.some(c=>Array.isArray(c)&&c[0]==='toast'&&c[1].title===(command==='clipboard'?'No text in clipboard':'Unable to retrieve selection')));
  if(command==='selection'){selection=new Error('unavailable');calls=[];await submit({text:'',width:'3',copyResultToClipboard:false});assert(calls.some(c=>Array.isArray(c)&&c[0]==='toast'&&c[1].title==='Unable to retrieve selection'));}
 }
 const readFailure=structuredClone(calls);calls=[];clipboard='a b c';selection='a b c';copyError=true;
 await assert.rejects(()=>submit({text:'a b c',width:'2',copyResultToClipboard:true}));r=render();
 assert.equal(children().find(n=>n.type==='Form.Description')?.props.text,'a\nb\nc');
 assert(!calls.some(c=>Array.isArray(c)&&c[0]==='toast'&&c[1].style==='success'));
 const copyFailure=structuredClone(calls);copyError=false;calls=[];clipboard='   ';selection='   ';
 await submit({text:'   ',width:'2',copyResultToClipboard:false});assert(!children().some(n=>n.type==='Form.Description'));
 assert.equal([r.actions.props.children].flat().filter(Boolean).length,1);
 components.push({command,prefs,initial,invalid,noCopy,copy:copied,readFailure,copyFailure,emptyResult:structuredClone(calls)});
}
const out=path.resolve(import.meta.dirname,'../fixtures/oracle.json');
fs.writeFileSync(out,JSON.stringify({validation,engine,pure,components},null,2)+'\n');
console.log(`Original TS oracle: ${validation.length} widths, ${engine.length} texts, ${pure.length} pure cases, ${components.length} component/preference scenarios`);
