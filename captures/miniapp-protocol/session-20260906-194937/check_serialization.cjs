// 只执行原客户端 ByteArray、Message 和编码器，不启动客户端或网络。
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const root = path.resolve(__dirname, '../../..');
const acorn = require(path.join(root, 'wechat-miniapp/analysis-tools/wx-static/node_modules/acorn'));
const mainPath = 'wechat-miniapp/subpackages/main/main.min.js';
const egretPath = 'wechat-miniapp/subpackages/libs/egret.min.js';
const main = fs.readFileSync(path.join(root, mainPath), 'utf8');
const engine = fs.readFileSync(path.join(root, egretPath), 'utf8');
const context = vm.createContext({Uint8Array, DataView, ArrayBuffer, egret: {$error: (...args) => {throw Error(args.join(','));}}, __reflect: () => {}});
const engineBlock = acorn.parse(engine, {ecmaVersion: 'latest'}).body.filter(n => engine.slice(n.start,n.end).includes('t.ByteArray = i'));
assert.equal(engineBlock.length,1);
vm.runInContext(engine.slice(engineBlock[0].start,engineBlock[0].end),context);
let messageModule, encoder;
function walk(n) {
  if (!n || typeof n !== 'object') return;
  if (n.type === 'FunctionExpression' && n.params.length === 3 && main.slice(n.end-60,n.end).includes('t.Message = n')) messageModule = n;
  if (n.type === 'AssignmentExpression' && n.left.type === 'MemberExpression' && n.left.property.name === 'encodeMsg' && n.left.object.type === 'MemberExpression' && n.left.object.property.name === 'prototype') encoder = n.right;
  for (const v of Object.values(n)) if (Array.isArray(v)) v.forEach(walk); else if (v && typeof v === 'object') walk(v);
}
walk(acorn.parse(main, {ecmaVersion:'latest'}));
assert(messageModule && encoder);
const exportsObject = {};
vm.runInContext('('+main.slice(messageModule.start,messageModule.end)+')',context)({}, exportsObject, id => {
  assert.equal(id,227);
  return {default: {getProtobufCls: () => null}};
});
const Message = exportsObject.Message, encode = vm.runInContext('('+main.slice(encoder.start,encoder.end)+')',context);
const sessionDir = process.argv[2] ? path.resolve(process.argv[2]) : __dirname;
const timeline = fs.readFileSync(path.join(sessionDir,'timeline.jsonl'),'utf8').trim().split('\n').map(JSON.parse);
const frames = [];
for (const row of timeline.filter(r => r.messageType === 12502 && r.direction === 'request')) {
  const raw = fs.readFileSync(path.join(sessionDir,row.rawFile));
  assert.equal(crypto.createHash('sha256').update(raw).digest('hex'),row.sha256);
  assert.equal(raw.readUInt16BE(0),raw.length);
  assert.equal(raw[24],0); assert.equal(raw[25],0);
  const msg = new Message(12502);
  for (const byte of raw.subarray(4,27)) msg.putByte(byte);
  let pos = 27;
  for (let i=0;i<raw[26];i++) {
    const size = raw.readUIntBE(pos,3); pos+=3;
    assert(pos+size <= raw.length);
    msg.putBytes(new context.egret.ByteArray(new Uint8Array(raw.subarray(pos,pos+size))));
    pos+=size;
  }
  assert.equal(msg.ba.position,pos-4);
  assert.equal(msg.ba.length,msg.ba.position);
  assert.equal(msg.bytes.length-msg.ba.length,4);
  const rebuilt = Buffer.from(encode(msg));
  assert.deepEqual(rebuilt,raw);
  const changed = Buffer.from(raw); changed[changed.length-1]=1;
  assert.notDeepEqual(rebuilt,changed);
  frames.push({sequence:row.sequence,source:row.rawFile,sha256:row.sha256,logicalBodyLength:msg.ba.length,allocatedBodyLength:msg.bytes.length,paddingOffset:pos,paddingHex:raw.subarray(pos).toString('hex'),exactRebuild:true});
}
if (sessionDir === __dirname) assert.equal(frames.length,7);
else assert(frames.length > 0);
// 证明尾长来自最后一个计划的分配余量，而非固定协议 int 字段。
const probes = [0,1,4,8].map(size => {
  const msg = new Message(12502);
  for (let i=0;i<23;i++) msg.putByte(0);
  msg.putBytes(new context.egret.ByteArray(new Uint8Array(size).fill(2)));
  assert.equal(msg.ba.length,size+26);
  assert.equal(msg.bytes.length,2*size+26);
  return {planLength:size,logicalLength:msg.ba.length,allocatedLength:msg.bytes.length};
});
const sources = [mainPath,egretPath].map(file=>({file,sha256:crypto.createHash('sha256').update(fs.readFileSync(path.join(root,file))).digest('hex')}));
process.stdout.write(JSON.stringify({sources,frames,probes}));
