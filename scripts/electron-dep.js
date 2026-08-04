// electron must live in "dependencies" for npm consumers (bin launcher needs it
// installed), but electron-builder refuses to build unless it is in
// "devDependencies". scripts/dist.js moves it out for the packaged build and
// restores it afterwards, so the repo and the published tarball keep it as a
// regular dependency.
const fs = require('fs')
const path = require('path')

const file = path.join(__dirname, '..', 'package.json')
const pkg = JSON.parse(fs.readFileSync(file, 'utf8'))

if (process.argv[2] === 'check') {
  if (!pkg.dependencies || !pkg.dependencies.electron) {
    console.error(
      'electron is not in "dependencies" — publishing now would ship a broken package.\n' +
        'A dist build probably died mid-way; run `node scripts/electron-dep.js to-deps` to restore.'
    )
    process.exit(1)
  }
  process.exit(0)
}

const [from, to] =
  process.argv[2] === 'to-deps'
    ? ['devDependencies', 'dependencies']
    : ['dependencies', 'devDependencies']

if (pkg[from] && pkg[from].electron) {
  pkg[to] = pkg[to] || {}
  pkg[to].electron = pkg[from].electron
  delete pkg[from].electron
  for (const section of ['dependencies', 'devDependencies']) {
    pkg[section] = Object.fromEntries(
      Object.entries(pkg[section]).sort(([a], [b]) => a.localeCompare(b))
    )
  }
  fs.writeFileSync(file, JSON.stringify(pkg, null, 2) + '\n')
}
