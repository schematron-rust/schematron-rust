<schema xmlns="http://purl.oclc.org/dsdl/schematron">
  <pattern id="broad">
    <rule context="*"><assert test="false()">broad</assert></rule>
  </pattern>
  <pattern id="narrow">
    <rule context="a"><assert test="false()">narrow</assert></rule>
  </pattern>
</schema>
