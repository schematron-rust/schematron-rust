<schema xmlns="http://purl.oclc.org/dsdl/schematron">
  <pattern>
    <rule context="line">
      <assert test="number(@qty) &gt; 0" diagnostics="qty-help units-help">Bad quantity.</assert>
    </rule>
  </pattern>
  <diagnostics>
    <diagnostic id="qty-help">Found <value-of select="@qty"/>.</diagnostic>
    <diagnostic id="units-help">Quantity is a count of units.</diagnostic>
  </diagnostics>
</schema>
