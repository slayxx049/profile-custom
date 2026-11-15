/**
 * SENSEX live-quote proxy
 * Read-only educational display. No trading or order execution.
 * Upstream endpoint documented by Praglitch/market-data-feed (MIT):
 * https://github.com/Praglitch/market-data-feed
 * BSE India live SENSEX quote endpoint:
 * https://api.bseindia.com/RealTimeBseIndiaAPI/api/GetSensexData/w
 *
 * BSE controls availability, update frequency and any redistribution terms.
 * This is not an exchange-certified or broker-grade feed.
 */
const UPSTREAM = "https://api.bseindia.com/RealTimeBseIndiaAPI/api/GetSensexData/w";

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

function number(value) {
  if (value === null || value === undefined || value === "") return null;
  const parsed = Number(String(value).replace(/,/g, "").trim());
  return Number.isFinite(parsed) ? parsed : null;
}

function normalizeRow(payload) {
  if (Array.isArray(payload)) return payload[0] || null;
  if (!payload || typeof payload !== "object") return null;
  for (const key of ["Data", "data", "Table", "Table1", "Result", "result"]) {
    if (Array.isArray(payload[key])) return payload[key][0] || null;
  }
  return payload;
}

async function getLiveSensex() {
  const upstream = await fetch(UPSTREAM, {
    method: "GET",
    headers: {
      "Accept": "application/json, text/plain, */*",
      "Accept-Language": "en-IN,en-US;q=0.9,en;q=0.8",
      "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
      "Referer": "https://www.bseindia.com/",
      "Origin": "https://www.bseindia.com"
    },
    redirect: "follow",
    cache: "no-store"
  });

  if (!upstream.ok) {
    throw new Error(`BSE returned HTTP ${upstream.status}`);
  }

  const payload = await upstream.json();
  const row = normalizeRow(payload);
  if (!row) throw new Error("BSE returned an empty SENSEX quote");

  const fields = Object.fromEntries(
    Object.entries(row).map(([key, value]) => [
      key.toLowerCase().replace(/[^a-z0-9]/g, ""),
      value
    ])
  );
  const pick = (...keys) => {
    for (const key of keys) {
      if (fields[key] !== undefined && fields[key] !== null && fields[key] !== "") {
        return fields[key];
      }
    }
    return null;
  };

  const price = number(pick("ltp", "lastprice", "lasttradedprice"));
  const previous = number(pick("prevclose", "previousclose", "prevclosing"));
  if (price === null || previous === null || previous === 0) {
    throw new Error("BSE response did not contain usable LTP and previous-close values");
  }

  const change = number(pick("chg", "change")) ?? (price - previous);
  const changePercent = number(pick("perchg", "changepercent", "percentchange"))
    ?? ((change / previous) * 100);
  const fetchedAt = new Intl.DateTimeFormat("en-IN", {
    timeZone: "Asia/Kolkata",
    day: "2-digit", month: "short", year: "numeric",
    hour: "2-digit", minute: "2-digit", second: "2-digit",
    hour12: true
  }).format(new Date()) + " IST";

  return {
    symbol: "^BSESN",
    price,
    previous_close: previous,
    change,
    change_percent: changePercent,
    open: number(pick("iopen", "open")),
    high: number(pick("high", "dayhigh")),
    low: number(pick("low", "daylow")),
    market_state: "BSE LIVE QUOTE",
    market_time: String(pick("dttm", "datetime", "markettime") ?? "Not supplied by BSE"),
    fetched_at: fetchedAt,
    history: [],
    source: "BSE India RealTimeBseIndiaAPI",
    feed_type: "BSE LIVE FEED",
    note: "Provider timing and availability are not guaranteed; not an exchange-certified or broker-grade feed."
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
      return json(await getLiveSensex());
    } catch (error) {
      return json({
        error: "Unable to retrieve the latest BSE SENSEX quote",
        detail: String(error?.message || error),
        fetched_at: new Date().toISOString()
      }, 502);
    }
  }
};
