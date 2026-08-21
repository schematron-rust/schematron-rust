<schema xmlns="http://purl.oclc.org/dsdl/schematron">
  <pattern>
    <rule context="item">
      <assert test="@id">An item needs an id.</assert>
    </rule>
  </pattern>
</schema>
