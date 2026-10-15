// The existing NovaSheet snapshot contains compiled jsxDEV calls. In a production
// build their first three arguments are equivalent to jsx; debug-only arguments
// are discarded. Keep this compatibility boundary local to the vendor importer.
export { jsx as jsxDEV, Fragment } from 'react/jsx-runtime';
