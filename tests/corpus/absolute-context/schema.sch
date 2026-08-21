<schema xmlns="http://purl.oclc.org/dsdl/schematron">
  <pattern>
    <rule context="/root/a">
      <assert test="false()">only the top-level a</assert>
    </rule>
  </pattern>
</schema>
