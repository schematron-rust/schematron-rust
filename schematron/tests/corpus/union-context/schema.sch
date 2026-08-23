<schema xmlns="http://purl.oclc.org/dsdl/schematron">
  <pattern>
    <rule context="a | b">
      <assert test="false()">matched <name/></assert>
    </rule>
  </pattern>
</schema>
