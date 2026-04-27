import * as fs from 'node:fs';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';

const REPO_ROOT = path.resolve(fileURLToPath(import.meta.url), '../..');
const modelSrc = path.join(REPO_ROOT, 'dist', 'model');
const modelDst = path.join(REPO_ROOT, 'npm', '@coree-ai', 'coree-model-bge-small-en-v1.5', 'model');

if (!fs.existsSync(modelSrc)) {
  console.error(`Missing model: ${modelSrc}`);
  process.exit(1);
}
fs.cpSync(modelSrc, modelDst, { recursive: true });
console.log('Bundled model into model package.');
