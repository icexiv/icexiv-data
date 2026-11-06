export default `
  :host { display:block; min-width:0; color:var(--fg, light-dark(#25312c,#e0e6e1)); font-family:var(--font-ui,sans-serif); }
  * { box-sizing:border-box; }
  .filters { display:flex; flex-wrap:wrap; gap:.4rem; margin:0 0 1.5rem; }
  button { font:inherit; font-size:.85em; color:inherit; background:transparent; border:1px solid var(--line,light-dark(#d9dfda,#3b453e)); border-radius:2rem; padding:.3rem .8rem; cursor:pointer; }
  button[aria-pressed="true"] { background:var(--fg,light-dark(#25312c,#e0e6e1)); color:var(--bg,light-dark(#fff,#1c211e)); border-color:transparent; }
  button:focus-visible, a:focus-visible, summary:focus-visible { outline:2px solid var(--accent,#638b75); outline-offset:4px; }
  section { margin:0 0 1.6rem; }
  h2 { margin:0 0 .65rem; font-size:.9em; font-weight:600; color:var(--muted,light-dark(#626e65,#a9b4ac)); }
  .grid { display:grid; grid-template-columns:repeat(auto-fit,minmax(min(100%,17rem),1fr)); gap:.65rem; }
  article { min-width:0; padding:1rem; border-radius:.65rem; background:var(--surface,light-dark(#f3f5f2,#242c26)); }
  header { display:flex; align-items:baseline; justify-content:space-between; gap:.6rem; }
  h3 { margin:0; font-size:1em; font-weight:650; }
  a { color:inherit; text-decoration:none; }
  a:hover { text-decoration:underline; text-underline-offset:.2em; }
  .badge { flex-shrink:0; display:inline-flex; align-items:center; gap:.35em; font-size:.75em; color:var(--muted,light-dark(#626e65,#a9b4ac)); }
  .badge::before { content:""; width:.5em; height:.5em; border-radius:50%; background:currentColor; }
  .ready { color:light-dark(#307047,#91c4a0); }
  .partial { color:light-dark(#8a601c,#dec084); }
  p { margin:.5rem 0 0; font-size:.9em; line-height:1.65; overflow-wrap:anywhere; }
  details { margin-top:.7rem; font-size:.85em; color:var(--muted,light-dark(#626e65,#a9b4ac)); }
  summary { cursor:pointer; width:fit-content; }
  details p { font-size:1em; }
  details a { display:inline-block; margin-top:.5rem; color:var(--accent,light-dark(#38654e,#a5c7ad)); }
  .error { color:light-dark(#993c30,#ffb4a8); }
  [hidden] { display:none !important; }
`;
