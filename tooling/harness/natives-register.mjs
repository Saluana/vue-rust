// Registers natives-hooks.mjs for a project oracle run. The natives come from the VN_NATIVES environment variable
// (JSON of native.json; "{}" when the project has none).
import { register } from 'node:module';

register('./natives-hooks.mjs', { parentURL: import.meta.url, data: { natives: JSON.parse(process.env.VN_NATIVES || '{}') } });
