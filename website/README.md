# Ely Tea Party website

The public introduction, built-in character previews and release downloads at https://elyteaparty.com. Static HTML, CSS and JavaScript; no framework, visitor accounts, cookies or external font dependencies. The avatar marketplace and binary/update hosting migration are separate follow-ups.

## Development

Run `npm run dev` here, then open http://localhost:8787. `npm run build` writes `dist/`, copying the actual application artwork and screenshot from the repository. Node.js 22+ is recommended. Generated output and Wrangler state are ignored.

## Release links

`release.json` records the verified public release. After publishing a new app version, update its version and asset names, verify the public files, rebuild and deploy. Downloads currently go to GitHub Releases; the website does not claim R2 hosting or alter the desktop updater.

## Deployment

`npm run deploy` publishes through Wrangler to the owner's Cloudflare account, with custom domains `elyteaparty.com` and `www.elyteaparty.com`. Use Wrangler's existing OAuth login; do not place credentials in this repository. The Worker serves static assets only. Check both public hosts, the 404 status, character state buttons, pause/play, narrow screens and release links after deployment.

The browser pauses character previews when the system requests reduced motion, and offers a manual pause button. Characters and app artwork retain the project's attribution. The page links to ELYTerminal and includes the MIT license.
