<schema xmlns="http://purl.oclc.org/dsdl/schematron" defaultPhase="basic">
  <phase id="basic"><active pattern="structure"/></phase>
  <phase id="strict"><active pattern="structure"/><active pattern="business"/></phase>
  <pattern id="structure">
    <rule context="a"><assert test="false()">structure</assert></rule>
  </pattern>
  <pattern id="business">
    <rule context="a"><assert test="false()">business</assert></rule>
  </pattern>
</schema>
