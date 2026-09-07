// 复用原小游戏的 protobuf 运行库及生成代码，校验本轮原帧往返一致。
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const root = path.resolve(__dirname, '../../..');
const protobuf = require(path.join(root, 'wechat-miniapp/subpackages/libs/protobuf.min.js'));
const context = vm.createContext({protobuf, window: {}});
vm.runInContext(fs.readFileSync(path.join(root, 'wechat-miniapp/subpackages/main/protobuf-bundle.min.js'), 'utf8'), context);
const types = {25013: 's2c_ResUpdate', 32109: 's2c_HandBookPowerInfo', 20300: 'C2SBuddy', 20301: 'S2CBuddy'};
const sessionDir = process.argv[2] ? path.resolve(process.argv[2]) : __dirname;
const rows = fs.readFileSync(path.join(sessionDir, 'timeline.jsonl'), 'utf8').trim().split('\n').map(JSON.parse);
const result = [];
for (const row of rows) {
  if (!types[row.messageType]) continue;
  const raw = fs.readFileSync(path.join(sessionDir, row.rawFile));
  assert.equal(crypto.createHash('sha256').update(raw).digest('hex'), row.sha256);
  const body = raw.subarray(4), type = context.pb[types[row.messageType]];
  const reader = protobuf.Reader.create(body), data = type.decode(reader);
  assert.equal(reader.pos, body.length);
  assert.deepEqual(Buffer.from(type.encode(data).finish()), body);
  assert.throws(() => {
    const short = body.subarray(0, -1), r = protobuf.Reader.create(short), d = type.decode(r);
    assert.equal(r.pos, short.length);
    assert.deepEqual(Buffer.from(type.encode(d).finish()), short);
  });
  result.push({sequence: row.sequence, source: row.rawFile, sha256: row.sha256, schema: types[row.messageType], data});
}
if (sessionDir === __dirname) assert.equal(result.length, 40);
else assert(result.length > 0);
process.stdout.write(JSON.stringify(result));
