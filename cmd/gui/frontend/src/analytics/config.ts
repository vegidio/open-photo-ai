// GA4 analytics config. Neither value is secret — both are client-side analytics identifiers (low sensitivity; worst
// case for the API secret is event spam to the property). If you prefer to inject them at release time, replace the
// literals with placeholder tokens and rewrite them in the release pipeline (cf. `<version>` in `shared/constants.go`).

// GA4 web data stream Measurement ID (Firebase console → Project settings → Web app, or GA4 Admin → Data Streams).
export const measurementId = 'G-1CPEG0M6TK';

// GA4 Measurement Protocol API secret — needed to POST events from the desktop webview (the Firebase web SDK's gtag
// transport doesn't work from the wails:// custom-scheme origin). Create at: GA4 Admin → Data Streams → your web
// stream → Measurement Protocol API secrets → Create.
export const measurementApiSecret = 'g_mw8pl_SimhBI2vcQUmcA';
