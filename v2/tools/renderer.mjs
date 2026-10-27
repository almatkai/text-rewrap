// SPDX-License-Identifier: MIT
// Execute actual beta Form parser, reconciliation and render/event functions.
import fs from 'node:fs';import vm from 'node:vm';import assert from 'node:assert/strict';import {transformSync} from 'esbuild';
const file=new URL('../fixtures/beta/form-runtime.tsx',import.meta.url);
const states=[],refs=[];let cursor=0,rc=0,effects=[];
const h=(type,props)=>({type,props:props??{}});
const react={forwardRef:fn=>fn,useMemo:fn=>fn(),useCallback:fn=>fn,useState(init){const i=cursor++;if(!(i in states))states[i]=typeof init==='function'?init():init;return [states[i],v=>states[i]=typeof v==='function'?v(states[i]):v]},useRef(init){const i=rc++;return refs[i]??={current:init}},useEffect:fn=>effects.push(fn),useImperativeHandle(ref,fn){ref.current=fn()}};
const module={exports:{}};
const code=transformSync(fs.readFileSync(file,'utf8')+'\nexport {collectFormItems};',{loader:'tsx',format:'cjs',jsx:'automatic'}).code;
vm.runInNewContext(code,{module,exports:module.exports,require(name){if(name==='react')return react;if(name==='react/jsx-runtime')return {jsx:h,jsxs:h};if(name.endsWith('detail-runtime'))return {parseNativeColorWheel:()=>null};return new Proxy({},{get:(_,key)=>String(key)})},window:{addEventListener(){},removeEventListener(){},clearTimeout(){},tezbar:{}},HTMLTextAreaElement:class {}},{filename:String(file)});
const {collectFormItems,reconcileFormValues,FormRuntime}=module.exports;
const fixtures=JSON.parse(fs.readFileSync(new URL('../fixtures/native-views.json',import.meta.url)));
const plain=value=>JSON.parse(JSON.stringify(value));
const fieldName=id=>id.split(':')[0];
const fields=root=>collectFormItems(root).filter(n=>n.type==='field').map(n=>n.field);
for(const view of fixtures){const f=fields(view.root);assert.equal(f.find(n=>fieldName(n.id)==='width').type,'Form.TextField');assert.equal(f.find(n=>fieldName(n.id)==='copyResultToClipboard').type,'Form.Checkbox');assert.equal(view.actions[0].kind,'submit-form');}
let launch=fixtures.find(v=>v.root.children.some(n=>n.type==='Form.TextArea')&&v.actions.length===1);
let invalid=fixtures.find(v=>v.root.children.some(n=>n.props?.error));
let output=fixtures.find(v=>v.actions.length===2);
assert(output.root.children.some(n=>n.type==='Form.Description'&&n.props.title==='Rewrapped Text'));
const f0=fields(launch.root),fi=fields(invalid.root),fo=fields(output.root);
const tid=f0.find(n=>fieldName(n.id)==='text').id,wid=f0.find(n=>fieldName(n.id)==='width').id,cid=f0.find(n=>fieldName(n.id)==='copyResultToClipboard').id;
const draft={[tid]:'newer queued 🦀\ntext',[wid]:'99junk',[cid]:false};
assert.deepEqual(plain(reconcileFormValues(fi,draft,f0)),draft);
assert.deepEqual(plain(reconcileFormValues(fo,draft,fi)),draft);
assert(fi.find(n=>fieldName(n.id)==='width').error);
const relaunched=f0.map(f=>({...f,id:f.id.replace(':EPOCH',':NEXT'),key:f.key.replace(':EPOCH',':NEXT')}));assert(!Object.values(reconcileFormValues(relaunched,draft,f0)).includes(draft[tid]));
const changed=[];let submitted;const ref={current:null};
let root=launch.root;
function render(){cursor=0;rc=0;effects=[];const tree=FormRuntime({root,title:launch.title,onBack(){},onSubmitForm:v=>submitted=v,onChangeField:(...args)=>changed.push(args),onInvokeAction(){},onOpenActions(){}},ref);for(const e of effects)e();return tree;}
function all(tree){if(!tree||typeof tree!=='object')return [];return [tree,...[tree.props?.children].flat(Infinity).flatMap(all)]}
let tree=render();tree=render();
let area=all(tree).find(n=>n.type==='textarea');assert(area);area.props.onChange({target:{value:draft[tid]}});
let check=all(tree).find(n=>n.type==='input'&&n.props.type==='checkbox');check.props.onChange({target:{checked:false}});
let width=all(tree).find(n=>n.type==='input'&&n.props.placeholder?.includes('Leave blank'));width.props.onChange({target:{value:draft[wid]}});
tree=render();assert.deepEqual(plain(ref.current.getValues()),draft);assert.equal(changed.length,0);
root=invalid.root;render();tree=render();assert.deepEqual(plain(ref.current.getValues()),draft);
all(tree).find(n=>n.type==='form').props.onSubmit({preventDefault(){}});assert.deepEqual(plain(submitted),draft);
root=output.root;render();render();assert.deepEqual(plain(ref.current.getValues()),draft);
// Beta controlled checkbox/dropdown events carry only {value}; test real update/rejection
// semantics alongside submit-only TextArea/Width so unrelated client drafts survive.
root=structuredClone(output.root);root.children.find(n=>n.props?.id===cid).props.actionId='controlled-check';
root.children.push({type:'Form.Dropdown',props:{id:'test-dropdown',title:'Test',value:'a',actionId:'controlled-dropdown',data:[{title:'A',value:'a'},{title:'B',value:'b'}]}});
render();tree=render();check=all(tree).find(n=>n.type==='input'&&n.props.type==='checkbox');check.props.onChange({target:{checked:false}});all(tree).find(n=>n.type==='select').props.onChange({target:{value:'b'}});
assert.deepEqual(plain(changed),[['controlled-check',false],['controlled-dropdown','b']]);
render();root=structuredClone(root);root.children.find(n=>n.props?.id===cid).props.value=true;root.children.find(n=>n.props?.id==='test-dropdown').props.value='a';render();render();
assert.equal(ref.current.getValues()[tid],draft[tid]);assert.equal(ref.current.getValues()[wid],draft[wid]);assert.equal(ref.current.getValues()[cid],true);assert.equal(ref.current.getValues()['test-dropdown'],'a');
const index=fs.readFileSync(new URL('../fixtures/beta/index.tsx',import.meta.url),'utf8');const overlay=fs.readFileSync(new URL('../fixtures/beta/action-runtime-overlay.tsx',import.meta.url),'utf8');
assert(index.includes("onInvokeAction(actionId, { value })"));assert(!overlay.includes('action.shortcut'));
console.log(`Beta Form parser/render/reconciliation: ${fixtures.length} native views, TextArea/Checkbox/Description/actions, queued drafts and controlled checkbox/dropdown rejection passed. Beta overlay does not bind cmd+c.`);
