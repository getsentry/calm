#!/usr/bin/env node
const { spawn } = require('child_process');

let args = ['-f', __dirname + '/eslintformatter.js'];

if (process.argv.length <= 2) {
  args.push('.');
} else {
  Array.prototype.push.apply(args, process.argv.slice(2));
}

let child = spawn('eslint', args, {
  stdio: 'inherit'
});

child.on('error', function(err) {
  console.error('error: failed to invoke eslint');
  console.error(err.stack);
});

child.on('exit', function(code) {
  process.exit(code);
});

process.on('SIGTERM', function() {
  child.kill('SIGTERM');
});

process.on('SIGINT', function() {
  child.kill('SIGINT');
});
