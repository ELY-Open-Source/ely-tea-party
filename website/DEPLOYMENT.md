# Website delivery

Deployed 2026-10-28 (UTC) to the owner's Cloudflare account.

- Primary URL: https://elyteaparty.com
- Additional host: https://www.elyteaparty.com
- Worker: `ely-tea-party-website`
- Cloudflare version: `a05dc33d-e087-421a-a30f-02ea3567c0f1`
- App release linked by the page: `v0.3.14`

## Verified

- Static build and JavaScript syntax checks pass; handwritten files remain below 500 lines.
- Local HTTP checks passed for 41 page/resource endpoints; internal anchors and image references resolve, security headers are present, and missing paths return HTTP 404.
- All three public GitHub binary URLs, the release page and checksum file return HTTP 200.
- Cloudflare confirms both custom domains target this Worker's production environment.
- Public DNS at 1.1.1.1 and 8.8.8.8 returns Cloudflare addresses for the new hosts.
- HTTPS responses for both hosts, CSS, JavaScript, a character animation, the screenshot, robots.txt and sitemap match the local build byte for byte. TLS certificate validation succeeds. The public missing-page response is HTTP 404.

## Verification boundaries

The local system resolver still cached the domain's earlier missing DNS record during verification. HTTPS checks therefore used `curl --resolve` with the IP returned by public DNS, retaining the real host name and normal certificate validation.

Browser access to the local preview was rejected because permission was declined. Browser rendering, responsive layout and interactive click behavior have not been visually verified. No alternate browser route was used.

The avatar marketplace, upload/review service, R2 binary hosting and desktop updater migration are not part of this first website delivery. Download links point directly to the existing GitHub release; the desktop app is unchanged.
