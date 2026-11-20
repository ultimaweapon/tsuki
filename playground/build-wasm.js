import { execFileSync, spawn } from 'node:child_process';
import { createInterface } from 'node:readline';

// Spawn cargo.
const cargo = spawn(
  'cargo',
  [
    'build',
    '-p',
    'tsuki-playground',
    '--target',
    'wasm32-unknown-unknown',
    '-r',
    '--message-format',
    'json-render-diagnostics',
  ],
  {
    stdio: ['ignore', 'pipe', 'pipe'],
  },
);

cargo.stderr.setEncoding('utf8');

// Read output.
let success, bin;
let rl = createInterface({
  input: cargo.stdout,
  crlfDelay: Infinity,
});

for await (const line of rl) {
  const msg = JSON.parse(line);

  switch (msg.reason) {
    case 'compiler-artifact':
      const url = new URL(msg.package_id);

      if (url.hash.startsWith('#tsuki-playground@')) {
        bin = msg.filenames[0];
      }

      break;
    case 'build-finished':
      success = msg.success;
      break;
  }
}

if (!success) {
  console.error(cargo.stderr.read());
  process.exit(1);
} else if (!bin) {
  console.error('Missing binary path from Cargo.');
  process.exit(1);
}

// Generate wrapper.
execFileSync('wasm-bindgen', [
  '--browser',
  '--out-name',
  'rust',
  '--out-dir',
  import.meta.dirname,
  bin,
]);
