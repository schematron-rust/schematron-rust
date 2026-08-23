<schema xmlns="http://purl.oclc.org/dsdl/schematron" defaultPhase="basic">
  <phase id="basic"><active pattern="structure"/></phase>
  <pattern id="structure">
    <rule context="a"><assert test="false()">structure</assert></rule>
  </pattern>
  <pattern id="unlisted">
    <rule context="a"><assert test="false()">unlisted by every phase</assert></rule>
  </pattern>
</schema>
