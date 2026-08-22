<schema xmlns="http://purl.oclc.org/dsdl/schematron">
  <!-- include takes the element itself. -->
  <include href="lib.sch#totals"/>

  <pattern id="local">
    <rule context="invoice">
      <assert test="line">needs a line</assert>
      <!-- extends href takes the children of the element, not the
           element, and splices them at its own position. -->
      <extends href="lib.sch#dated"/>
      <assert test="currency">needs a currency</assert>
    </rule>
  </pattern>
</schema>
