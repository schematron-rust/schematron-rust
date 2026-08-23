<schema xmlns="http://purl.oclc.org/dsdl/schematron">
  <pattern>
    <rule context="@x"><assert test="false()">attribute <name/></assert></rule>
    <rule context="text()"><assert test="false()">text</assert></rule>
    <rule context="comment()"><assert test="false()">comment</assert></rule>
    <rule context="processing-instruction()"><assert test="false()">pi</assert></rule>
  </pattern>
</schema>
