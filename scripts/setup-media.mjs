import { mkdirSync, copyFileSync, existsSync } from 'node:fs';
import { resolve, dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'..');
const tools=join(root,'tools');const packages=join(tools,'.packages');mkdirSync(packages,{recursive:true});
const npm=process.env.npm_execpath;
if (!npm) throw new Error('请使用 npm run setup:media 运行。');
const install=spawnSync(process.execPath,[npm,'install','--prefix',packages,'--no-save','--no-package-lock','ffmpeg-static@5.3.0','ffprobe-static@3.1.0'],{stdio:'inherit',windowsHide:true});
if(install.status!==0)process.exit(install.status??1);
const extension=process.platform==='win32'?'.exe':'';
const ffmpeg=join(packages,'node_modules/ffmpeg-static',`ffmpeg${extension}`);
const ffprobe=join(packages,'node_modules/ffprobe-static/bin',process.platform,process.arch,`ffprobe${extension}`);
if (!existsSync(ffmpeg)||!existsSync(ffprobe))throw new Error('媒体工具下载未完成，请重试。');
copyFileSync(ffmpeg,join(tools,`ffmpeg${extension}`));copyFileSync(ffprobe,join(tools,`ffprobe${extension}`));
for(const name of ['LICENSE','LICENSE.txt','ffmpeg.LICENSE','README.md']){const src=join(packages,'node_modules/ffmpeg-static',name);if(existsSync(src))copyFileSync(src,join(tools,`ffmpeg-static-${name}`));}
for(const name of ['LICENSE','README.md']){const src=join(packages,'node_modules/ffprobe-static',name);if(existsSync(src))copyFileSync(src,join(tools,`ffprobe-static-${name}`));}
for(const [source,target] of [[`ffmpeg${extension}.LICENSE`,'FFmpeg-LICENSE.txt'],[`ffmpeg${extension}.README`,'FFmpeg-BUILD.txt']]){const src=join(packages,'node_modules/ffmpeg-static',source);if(existsSync(src))copyFileSync(src,join(tools,target));}
// The ffprobe-static package MIT license covers its wrapper; its shipped
// ffprobe binary reports GPL v3. Include the full GPL text separately.
copyFileSync(join(root,'LICENSE'),join(tools,'FFprobe-LICENSE-GPL-3.0.txt'));
console.log('FFmpeg 和 ffprobe 已准备到 tools/。开发版自动发现；分发时放在程序旁的 tools/。');
