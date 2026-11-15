/**
 * SENSEX quote proxy
 * Read-only educational display. No trading or order execution.
 * Data source: Yahoo Finance chart endpoint for ^BSESN.
 *
 * Yahoo's chart endpoint is unofficial and undocumented. Data may be delayed,
 * rate-limited, unavailable, or changed without notice. Not an exchange-certified
 * or broker-grade feed; check provider terms before redistribution.
 */
const UPSTREAM = "https://query1.finance.yahoo.com/v8/finance/chart/%5EBSESN?range=1d&interval=1m&includePrePost=false";

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

function formatIndiaTime(unixSeconds) {
  if (!unixSeconds) return "Not supplied by provider";
  return new Intl.DateTimeFormat("en-IN", {
    timeZone: "Asia/Kolkata",
    day: "2-digit", month: "short", year: "numeric",
    hour: "2-digit", minute: "2-digit", second: "2-digit",
    hour12: true
  }).format(new Date(unixSeconds * 1000)) + " IST";
}

async function getLiveSensex() {
  const upstream = await fetch(UPSTREAM, {
    method: "GET",
    headers: {
      "Accept": "application/json",
      "Accept-Language": "en-IN,en;q=0.9",
      "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36"
    },
    redirect: "follow"
  });

  if (!upstream.ok) {
    throw new Error("Yahoo Finance returned HTTP " + upstream.status);
  }

  const payload = await upstream.json();
  const chart = payload?.chart?.result?.[0];
  if (payload?.chart?.error) {
    throw new Error(payload.chart.error.description || "Yahoo Finance chart error");
  }
  if (!chart?.meta) throw new Error("Yahoo Finance returned no SENSEX chart metadata");

  const meta = chart.meta;
  const price = Number(meta.regularMarketPrice);
  const previous = Number(meta.chartPreviousClose ?? meta.previousClose);
  if (!Number.isFinite(price) || !Number.isFinite(previous) || previous <= 0) {
    throw new Error("Yahoo Finance response did not contain a usable price and previous close");
  }

  const change = price - previous;
  const changePercent = (change / previous) * 100;
  const quote = chart.indicators?.quote?.[0] || {};
  const timestamps = chart.timestamp || [];
  const closes = quote.close || [];
  const history = timestamps
    .map((time, index) => ({
      time: time,
      close: closes[index]
    }))
    .filter((point) => Number.isFinite(point.close))
    .slice(-120);

  const marketTime = formatIndiaTime(meta.regularMarketTime);
  return {
    symbol: "^BSESN",
    price: price,
    previous_close: previous,
    change: change,
    change_percent: changePercent,
    open: Number.isFinite(Number(meta.regularMarketOpen)) ? Number(meta.regularMarketOpen) : null,
    high: Number.isFinite(Number(meta.regularMarketDayHigh)) ? Number(meta.regularMarketDayHigh) : null,
    low: Number.isFinite(Number(meta.regularMarketDayLow)) ? Number(meta.regularMarketDayLow) : null,
    market_state: meta.marketState || "PROVIDER STATUS NOT SUPPLIED",
    market_time: marketTime,
    fetched_at: formatIndiaTime(Math.floor(Date.now() / 1000)),
    history: history,
    source: "Yahoo Finance chart endpoint (unofficial)",
    feed_type: "YAHOO FINANCE · DELAYED/UNOFFICIAL",
    note: "Quote timing may be delayed. This unofficial endpoint has no uptime guarantee and is not an exchange-certified or broker-grade feed."
  };
}

function escapeXml(value) {
  return String(value ?? "").replace(/[&<>"']/g, (char) => ({
    "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&apos;"
  })[char]);
}

function svgCard(data) {
  const positive = data.change >= 0;
  const accent = positive ? "#65f2b5" : "#ff718f";
  const sign = positive ? "+" : "";
  const price = data.price.toLocaleString("en-IN", { minimumFractionDigits: 2, maximumFractionDigits: 2 });
  const change = sign + data.change.toLocaleString("en-IN", { minimumFractionDigits: 2, maximumFractionDigits: 2 });
  const percent = sign + data.change_percent.toFixed(2) + "%";
  const timestamp = escapeXml(data.market_time);
  const fetched = escapeXml(data.fetched_at);
  const previous = data.previous_close.toLocaleString("en-IN", { minimumFractionDigits: 2, maximumFractionDigits: 2 });
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="900" height="260" viewBox="0 0 900 260" role="img" aria-label="SENSEX ${price}, ${change} points, ${percent}">
  <defs><linearGradient id="bg" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#0b1020"/><stop offset="1" stop-color="#1b1530"/></linearGradient></defs>
  <rect x="1" y="1" width="898" height="258" rx="16" fill="url(#bg)" stroke="#7df9ff" stroke-opacity=".55"/>
  <text x="28" y="36" fill="#7df9ff" font-family="Arial,sans-serif" font-size="13" font-weight="700" letter-spacing="2">SENSEX / MARKET WATCH</text>
  <text x="872" y="36" text-anchor="end" fill="#65f2b5" font-family="Arial,sans-serif" font-size="11" font-weight="700">YAHOO · UNOFFICIAL</text>
  <text x="28" y="120" fill="#f5f2ff" font-family="Arial,sans-serif" font-size="58" font-weight="700">${escapeXml(price)}</text>
  <text x="30" y="158" fill="${accent}" font-family="Arial,sans-serif" font-size="22" font-weight="700">${escapeXml(change)} (${escapeXml(percent)})</text>
  <text x="872" y="103" text-anchor="end" fill="#aaa6c8" font-family="Arial,sans-serif" font-size="12">S&amp;P BSE SENSEX · ^BSESN</text>
  <text x="872" y="127" text-anchor="end" fill="#aaa6c8" font-family="Arial,sans-serif" font-size="11">PREVIOUS CLOSE  ${escapeXml(previous)}</text>
  <path d="M28 185H872" stroke="#34334e"/>
  <text x="28" y="216" fill="#aaa6c8" font-family="Arial,sans-serif" font-size="11">MARKET TIME: ${timestamp}</text>
  <text x="872" y="216" text-anchor="end" fill="#aaa6c8" font-family="Arial,sans-serif" font-size="11">FETCHED: ${fetched}</text>
  <text x="28" y="242" fill="#ffcf70" font-family="Arial,sans-serif" font-size="10">QUOTE MAY BE DELAYED · UNOFFICIAL SOURCE · NOT A TRADING FEED</text>
  </svg>`;
  return new Response(svg, {
    headers: {
      "Content-Type": "image/svg+xml; charset=utf-8",
      "Access-Control-Allow-Origin": "*",
      "Cache-Control": "no-store"
    }
  });
}

export default {
  async fetch(request) {
    const url = new URL(request.url);
    if (request.method === "OPTIONS") return json({ ok: true });
    if (request.method !== "GET") return json({ error: "Method not allowed" }, 405);
    if (url.pathname !== "/" && url.pathname !== "/sensex" && url.pathname !== "/sensex.svg") {
      return json({ error: "Not found" }, 404);
    }
    try {
      const data = await getLiveSensex();
      if (url.pathname === "/sensex.svg") return svgCard(data);
      return json(data);
    } catch (error) {
      return json({
        error: "Unable to retrieve the latest Yahoo Finance SENSEX quote",
        detail: String(error?.message || error),
        fetched_at: new Date().toISOString()
      }, 502);
    }
  }
};
