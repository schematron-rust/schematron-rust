<schema xmlns="http://purl.oclc.org/dsdl/schematron">
  <ns prefix="inv" uri="http://example.com/invoice"/>
  <pattern>
    <rule context="inv:invoice">
      <assert test="inv:total">An invoice needs a total.</assert>
    </rule>
  </pattern>
</schema>
