# SENSEX minute-feed setup

This repository contains the GitHub Pages dashboard and the read-only Cloudflare Worker source. The Worker is a small API proxy: when the dashboard polls it once per minute, it requests the latest available Yahoo Finance chart snapshot for `^BSESN`.

## Deploy the feed

1. Sign in to Cloudflare and open **Workers & Pages**.
2. Create a Worker named `sensex-minute-feed`.
3. Open its code editor and replace the starter code with the complete contents of [worker.js](../cloudflare-worker/worker.js).
4. Deploy the Worker and copy its public `workers.dev` URL. The endpoint is the URL itself, or the same URL followed by `/sensex`.
5. In this repository, open [assets/feed-config.json](../assets/feed-config.json), choose **Edit**, and set the `endpoint` value to your Worker URL. Commit the change.
6. Open the [SENSEX dashboard](https://slayxx049.github.io/profile-custom/sensex-dashboard.html) and use **REFRESH SNAPSHOT**. The dashboard will then poll the Worker every 60 seconds.

Example configuration (replace the example URL with the URL Cloudflare gives your Worker):

```json
{
  "endpoint": "https://sensex-minute-feed.YOUR-SUBDOMAIN.workers.dev/sensex"
}
```

## Important data limitations

- The browser checks the Worker every 60 seconds. The Worker fetches the provider when requested; this is not an exchange-certified real-time feed.
- Yahoo Finance may delay, throttle, or repeat its quote. A one-minute request interval does **not** guarantee a new market price every minute.
- The feed is read-only and educational. It is not connected to a broker and cannot submit orders.
- The dashboard's bid/ask ladder remains simulated. SENSEX is an index, not an executable order book.
- If the Worker is not configured, the dashboard continues to use the repository's published snapshot.

No API key is embedded in the public repository. Do not put account passwords or private tokens in the Worker source or the README.
