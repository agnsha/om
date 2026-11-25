# Om website

The "how to use Om" site: install, first program, the Book, Om by Example,
exercises, tools, games and the reference. Plain static HTML built from the
Markdown in `content/`.

```sh
cd website
npm ci
npm run build        # writes dist/
npx serve dist       # or open dist/index.html
```

- `content/` is the Markdown. `content/docs/` holds copies of the repository's
  `docs/` pages; refresh them when those change.
- `static/` is copied as-is (stylesheet, images, Cloudflare `_headers`).
- `app/` holds the two React pieces on the home page (the glass Om logo intro and
  the black bat flower hero). esbuild bundles them into `dist/js/home.js`.
- `build.mjs` turns `.md` links into `.html`, links GitHub `docs/` URLs to the
  site's copies, and highlights `om` code blocks.
- Pages that describe work not merged yet start with an **In review** note.

## Deploying to Cloudflare Pages

The site is public even though the repository is private, so deploys are
manual. Either:

- **From your computer:** `npx wrangler login`, then
  `npx wrangler pages deploy dist --project-name om-lang` from `website/`.
- **From GitHub Actions:** add the repository secrets `CLOUDFLARE_API_TOKEN`
  (a token with *Cloudflare Pages: Edit*) and `CLOUDFLARE_ACCOUNT_ID`, then run
  the *Website (Cloudflare Pages)* workflow by hand.

Never commit a token.
