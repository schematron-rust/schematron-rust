<schema xmlns="http://purl.oclc.org/dsdl/schematron">
  <pattern>
    <rule context="invoice">
      <assert test="total">An invoice needs a total.</assert>
      <report test="@void">This invoice is void.</report>
    </rule>
  </pattern>
</schema>
