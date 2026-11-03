/**
 * SENSEX minute-feed proxy
 * Read-only educational data endpoint. It does not place orders.
 * Upstream: Yahoo Finance chart endpoint; availability and delay are provider-controlled.
 */
const SYMBOL = "%5EBSESN";
const UPSTREAM = `https://query1.finance.yahoo.com/v8/finance/chart/${SYMBOL}?range=1d&interval=1m`;

function ist(epochSeconds) {
  return new Intl.DateTimeFormat("en-IN", {
    timeZone: "Asia/Kolkata",
    day: "2-digit", month: "short", year: "numeric",
    hour: "2-digit", minute: "2-digit", second: "2-digit",
    hour12: true
  }).format(new Date(epochSeconds * 1000)) + " IST";
}

function json(data, status = 200) {
  return new Response(JSON.stringify(data), {
    status,
    headers: {
      "Content-Type": "application/json; charset=utf-8",
      "Access-Control-Allow-Origin": "*",
      "Access-Control-Allow-Methods": "GET, OPTIONS",
      "Access-Control-Allow-Headers": "Content-Type",
      "Cache-Control": "no-store"
    }
  });
}

async function getQuote() {
  const upstream = await fetch(UPSTREAM, {
    headers: { "Accept": "application/json" },
    cf: { cacheTtl: 0, cacheEverything: false }
  });
  if (!upstream.ok) throw new Error(`Upstream returned HTTP ${upstream.status}`);
  const payload = await upstream.json();
  const result = payload?.chart?.result?.[0];
  if (!result) throw new Error(payload?.chart?.error?.description || "No quote returned");
  const meta = result.meta || {};
  const price = Number(meta.regularMarketPrice);
  const previous = Number(meta.previousClose ?? meta.chartPreviousClose);
  if (!Number.isFinite(price) || !Number.isFinite(previous) || previous === 0) {
    throw new Error("The provider did not return a usable index quote");
  }
  const timestamps = result.timestamp || [];
  const closes = result.indicators?.quote?.[0]?.close || [];
  const history = timestamps.flatMap((ts, i) => {
    const close = Number(closes[i]);
    return Number.isFinite(close) ? [{
      time: new Intl.DateTimeFormat("en-IN", {
        timeZone: "Asia/Kolkata", hour: "2-digit", minute: "2-digit", hour12: false
      }).format(new Date(ts * 1000)),
      close
    }] : [];
  });
  const fetched = new Date();
  return {
    symbol: "^BSESN",
    price,
    previous_close: previous,
    change: price - previous,
    change_percent: ((price - previous) / previous) * 100,
    market_state: String(meta.marketState || "UNKNOWN").replaceAll("_", " "),
    market_time: meta.regularMarketTime ? ist(meta.regularMarketTime) : "Not supplied by source",
    fetched_at: ist(Math.floor(fetched.getTime() / 1000)),
    history,
    source: "Yahoo Finance chart endpoint; may be delayed"
  };
}

export default {
  async fetch(request) {
    const url = new URL(request.url);
    if (request.method === "OPTIONS") return json({ ok: true });
    if (request.method !== "GET") return json({ error: "Method not allowed" }, 405);
    if (url.pathname !== "/" && url.pathname !== "/sensex") {
      return json({ error: "Not found" }, 404);
    }
    try {
      return json(await getQuote());
    } catch (error) {
      return json({
        error: "Unable to retrieve the latest available SENSEX snapshot",
        detail: String(error?.message || error),
        fetched_at: new Date().toISOString()
      }, 502);
    }
  }
};
