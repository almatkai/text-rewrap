// SPDX-License-Identifier: MIT
import fs from 'node:fs';import assert from 'node:assert/strict';import crypto from 'node:crypto';import Ajv2020 from 'ajv/dist/2020.js';
const base=new URL('../',import.meta.url),read=p=>fs.readFileSync(new URL(p,base));const json=p=>JSON.parse(read(p));
const schema=json('fixtures/beta/extension-v2.schema.json');const manifest=json('extension-v2.json');
const validate=new Ajv2020({strict:false}).compile(schema);assert(validate(manifest),JSON.stringify(validate.errors));
assert(!validate({...manifest,author:'rembridge'}));assert.equal(manifest.id,'com.tezbar.native.text-rewrap');assert.deepEqual(manifest.replaces,['raycast.text-rewrap']);assert(manifest.verification.every(v=>v.state==='not-run'));
const original=json('fixtures/original/package.json');assert.deepEqual(manifest.preferences,original.preferences);
for(const [i,c] of manifest.commands.entries()){assert.equal(c.name,original.commands[i].name);assert.equal(c.title,original.commands[i].title);assert.equal(c.icon,'/extensions/icons/'+original.commands[i].icon.split('/').at(-1));}
for(const [file,p] of Object.entries(json('fixtures/provenance.json'))){assert.equal(crypto.createHash('sha256').update(read(file)).digest('hex'),p.sha256,file);assert.equal(p.commit,'fd300e276356587726566026f072563eb9c34cb1');}
const icons=json('fixtures/icons.json');for(const [file,p] of Object.entries(icons)){assert.equal(read('assets/'+file).length,p.bytes);assert.equal(crypto.createHash('sha256').update(read('assets/'+file)).digest('hex'),p.sha256);}
assert.equal(icons['text-rewrap.png'].sha256,'605539b2bad407e75ad6fad90363bdaf7dddf65a8ac2d1f2ea3865d93869523f');assert.equal(icons['text-rewrap.png'].bytes,3194);
console.log('Full beta JSON Schema, all command/preference metadata, beta provenance, all original PNG hashes passed.');
