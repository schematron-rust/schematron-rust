<schema xmlns="http://purl.oclc.org/dsdl/schematron">
  <pattern>
    <rule context="invoice" subject="total">
      <assert test="number(total) &gt; 0">The total must be positive.</assert>
      <assert test="@id" subject=".">The invoice needs an id.</assert>
    </rule>
  </pattern>
</schema>
