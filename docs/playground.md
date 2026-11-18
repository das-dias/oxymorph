# Playground

Convert a netlist between formats entirely in your browser. The conversion runs
in WebAssembly — the same Rust code that the tests prove equivalent to upstream
morphnet (see [Equivalence](equivalence.md)). Nothing is sent to a server.

<div class="oxymorph-playground" style="display:grid;gap:0.75rem;">
  <div style="display:flex;gap:1rem;flex-wrap:wrap;align-items:center;">
    <label style="display:flex;gap:0.4rem;align-items:center;">
      Input format
      <select id="ox-in" style="padding:0.25rem;"></select>
    </label>
    <label style="display:flex;gap:0.4rem;align-items:center;">
      Output format
      <select id="ox-out" style="padding:0.25rem;"></select>
    </label>
    <button id="ox-convert" style="padding:0.3rem 0.9rem;cursor:pointer;">Convert →</button>
  </div>
  <textarea id="ox-input" spellcheck="false" rows="14"
    style="width:100%;font-family:var(--md-code-font-family,monospace);font-size:0.8rem;padding:0.5rem;"></textarea>
  <textarea id="ox-output" spellcheck="false" rows="14" readonly
    style="width:100%;font-family:var(--md-code-font-family,monospace);font-size:0.8rem;padding:0.5rem;"></textarea>
  <p id="ox-status" style="margin:0;font-size:0.8rem;"></p>
</div>

<script type="module">
  import init, { convert, formats } from "../assets/playground/oxymorph_wasm.js";

  const example = `subckt rc_lowpass (in_ out gnd)
R1 (in_ out) resistor value=1k
C1 (out gnd) capacitor value=100p
ends rc_lowpass
`;

  const inSel = document.getElementById("ox-in");
  const outSel = document.getElementById("ox-out");
  const input = document.getElementById("ox-input");
  const output = document.getElementById("ox-output");
  const status = document.getElementById("ox-status");

  function convertNow() {
    try {
      output.value = convert(input.value, inSel.value, outSel.value);
      status.textContent = "";
    } catch (e) {
      output.value = "";
      status.textContent = "⚠️ " + String(e);
    }
  }

  try {
    await init();
    const names = formats();
    for (const f of names) {
      inSel.add(new Option(f, f));
      outSel.add(new Option(f, f));
    }
    input.value = example;
    inSel.value = "spectre";
    outSel.value = "vams";
    for (const el of [inSel, outSel, input]) {
      el.addEventListener("input", convertNow);
      el.addEventListener("change", convertNow);
    }
    document.getElementById("ox-convert").addEventListener("click", convertNow);
    convertNow();
  } catch (e) {
    status.textContent = "Failed to load the WebAssembly module: " + String(e);
  }
</script>

## Formats

Pick any input flavor and any output flavor; oxymorph parses into its shared IR
and writes it back out:

- `spice`
- `hspice`
- `xyce`
- `spectre`
- `spectre_spice`
- `vams` (Verilog-AMS)
