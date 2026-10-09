// Isolated local telemetry endpoint for Windows CI: no user data sent off runner.
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';

const directory = process.env.AQUILUM_RAM_OUT;
if (!directory) throw new Error('AQUILUM_RAM_OUT not provided');
fs.mkdirSync(directory, { recursive: true });
const eventFile = path.join(directory, 'events.jsonl');
const stageFile = path.join(directory, 'current-stage.txt');
fs.writeFileSync(stageFile, 'app_starting\n');
const server = http.createServer((req, res) => {
  res.setHeader('Access-Control-Allow-Origin', '*');
  res.setHeader('Access-Control-Allow-Headers', 'Content-Type');
  res.setHeader('Access-Control-Allow-Methods', 'GET, POST, OPTIONS');
  if (req.method === 'OPTIONS') { res.writeHead(204); res.end(); return; }
  if (req.method === 'GET' && req.url === '/health') {
    res.writeHead(200, { 'Content-Type': 'text/plain' }); res.end('ok'); return;
  }
  if (req.method !== 'POST' || req.url !== '/event') {
    res.writeHead(404); res.end(); return;
  }
  let body = '';
  req.on('data', (chunk) => {
    body += chunk.toString('utf8');
    if (body.length > 65000) req.destroy();
  });
  req.on('end', () => {
    try {
      const event = JSON.parse(body);
      if (!/^[a-z0-9_]+$/.test(event.stage)) throw new Error('Invalid stage');
      const record = { time: new Date().toISOString(), ...event };
      fs.appendFileSync(eventFile, JSON.stringify(record) + '\n');
      fs.writeFileSync(stageFile, event.stage + '\n');
      console.log('AQUILUM_UI_EVENT ' + JSON.stringify(record));
      res.writeHead(204); res.end();
    } catch (error) {
      console.error('Telemetry failed: ', error);
      res.writeHead(400); res.end('Bad event');
    }
  });
});
server.listen(18713, '127.0.0.1', () => console.log('AQUILUM_RAM_LISTENING port=18713'));
