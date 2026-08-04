// electron-builder refuses to build while electron sits in "dependencies", but
// npm consumers need it there. Move it to devDependencies for the build and
// restore afterwards, even when the build fails.
const { spawnSync } = require('child_process')
const path = require('path')

const dep = path.join(__dirname, 'electron-dep.js')
const run = (cmd, args) => spawnSync(cmd, args, { stdio: 'inherit', shell: false })

run(process.execPath, [dep, 'to-dev'])
const result = run('npx', ['electron-builder', ...process.argv.slice(2)])
run(process.execPath, [dep, 'to-deps'])
process.exit(result.status ?? 1)
