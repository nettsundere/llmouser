#!/usr/bin/env node
const { spawn } = require('child_process')
const path = require('path')
const electron = require('electron')

const child = spawn(electron, [path.join(__dirname, '..')], { stdio: 'inherit' })
child.on('close', (code) => process.exit(code ?? 0))
