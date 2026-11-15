# SENSEX live-feed setup

The dashboard can request a live SENSEX index quote from BSE India through a small Cloudflare Worker proxy. The Worker source is in [cloudflare-worker/worker.js](../cloudflare-worker/worker.js).

## Data source

The integration follows the live SENSEX endpoint documented by the MIT-licensed [Praglitch/market-data-feed](https://github.com/Praglitch/market-data-feed) project:

`GET https://api.bseindia.com/RealTimeBseIndiaAPI/api/GetSensexData/w`

The endpoint returns the current SENSEX quote fields (including LTP, change, percentage change, previous close, and the provider timestamp when supplied). This is an index quote, not an executable order book.

## Deploy the feed

1. Sign in to Cloudflare and open **Workers & Pages**.
2. Create a Worker named `sensex-live-feed`.
3. Open its code editor and replace the starter code with the complete contents of [worker.js](../cloudflare-worker/worker.js).
4. Deploy the Worker and copy its public `workers.dev` URL.
5. In this repository, open [assets/feed-config.json](../assets/feed-config.json), choose **Edit**, and set `endpoint` to the Worker URL followed by `/sensex`. Commit the change.
6. Open the [SENSEX dashboard](https://slayxx049.github.io/profile-custom/sensex-dashboard.html). It requests a new quote immediately and polls the Worker every 60 seconds while the page remains open.

Example configuration (replace the example URL with the URL Cloudflare gives your Worker):

```json
{
  "endpoint": "https://sensex-live-feed.YOUR-SUBDOMAIN.workers.dev/sensex"
}
```

## Verify the endpoint

Open the Worker URL with `/sensex`. A successful response should be JSON containing:

- `symbol`: `^BSESN`
- `price`, `previous_close`, `change`, and `change_percent`
- `market_time`: timestamp supplied by BSE, when available
- `fetched_at`: when the Worker retrieved the quote
- `source`: `BSE India RealTimeBseIndiaAPI`

If the endpoint returns HTTP 502, BSE may be unavailable, may have changed its response, or may be blocking the request. The dashboard will show the error rather than silently presenting an old quote as live.\n\nThe same Worker also exposes `/sensex.svg`, a dynamically rendered SVG market card. After deploying the Worker, you can use `https://YOUR-WORKER-URL/sensex.svg` as the README image source to show the latest quote in the GitHub profile. GitHub and browser image caches may still delay visual refreshes.

## Data limitations

- The upstream endpoint is documented as a live SENSEX quote endpoint, but BSE controls availability, update frequency, throttling, and timestamp quality. This project does not certify that every response is instantaneous or suitable for financial decisions.
- The dashboard polls every 60 seconds only while it is open. A poll does not guarantee that BSE has published a new value.
- The dashboard's rolling chart is built from quotes received during the open browser session; it is not historical intraday candle data.
- The bid/ask ladder, quantities, spread, and replay frames are simulated for interface demonstration. SENSEX is an index and does not have a directly executable order book.
- If the Worker is not configured, the dashboard continues to use the repository snapshot published by GitHub Actions (Yahoo Finance; may be delayed).
- The feed is read-only. It is not connected to a broker and cannot submit orders.

No API key is embedded in the public repository. Do not put account passwords or private tokens in the Worker source or the README.
