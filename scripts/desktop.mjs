import { existsSync, readdirSync } from 'node:fs';
import { dirname, resolve, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawn } from 'node:child_process';
const root=resolve(dirname(fileURLToPath(import.meta.url)), '..');
const env={...process.env};
const originalPath=env.Path||env.PATH||'';
delete env.Path;env.PATH=originalPath;
// The development session uses a workspace-contained toolchain. A normal clone uses PATH.
const portable=resolve(root,'../../work/toolchain');
if (process.platform==='win32' && existsSync(join(portable,'cargo/bin/cargo.exe'))) {
  env.CARGO_HOME=join(portable,'cargo');env.RUSTUP_HOME=join(portable,'rustup');
  const compiler=existsSync(join(portable,'mingw64/bin/gcc.exe'))?'mingw64':readdirSync(portable).find(n=>n.startsWith('llvm-mingw-')&&existsSync(join(portable,n,'bin')));
  env.PATH=[join(portable,'cargo/bin'),compiler?join(portable,compiler,'bin'):'',dirname(process.execPath),join(root,'src-tauri/target/debug'),env.PATH].filter(Boolean).join(';');
  env.RUSTFLAGS=[env.RUSTFLAGS||'','-C link-self-contained=yes'].join(' ').trim();
  if (compiler==='mingw64') {env.CC=join(portable,compiler,'bin/gcc.exe');env.CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=env.CC;}
}
const cli=join(root,'node_modules/@tauri-apps/cli/tauri.js');
const args=process.argv.slice(2);
const testing=args[0]==='test';
const child=spawn(testing?(process.platform==='win32'?'cargo.exe':'cargo'):process.execPath,testing?['test','--manifest-path','src-tauri/Cargo.toml','--no-default-features',...args.slice(1)]:[cli,...args],{cwd:root,env,stdio:'inherit',windowsHide:true});
child.on('error',e=>{console.error(e.message);process.exitCode=1;});
child.on('exit',code=>{process.exitCode=code??1;});
