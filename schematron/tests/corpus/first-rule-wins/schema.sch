<schema xmlns="http://purl.oclc.org/dsdl/schematron">
  <pattern>
    <rule context="*">
      <assert test="false()">claimed by the broad rule</assert>
    </rule>
    <rule context="a">
      <assert test="false()">this rule never runs</assert>
    </rule>
  </pattern>
</schema>
