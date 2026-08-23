<schema xmlns="http://purl.oclc.org/dsdl/schematron">
  <pattern>
    <rule context="line[@type='discount']">
      <assert test="number(@amount) &lt; 0">A discount must be negative.</assert>
    </rule>
    <rule context="line">
      <assert test="number(@amount) &gt;= 0">A normal line must not be negative.</assert>
    </rule>
  </pattern>
</schema>
