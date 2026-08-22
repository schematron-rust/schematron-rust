<schema xmlns="http://purl.oclc.org/dsdl/schematron">
  <!-- A library of reusable parts, addressed by fragment identifier.
       Only the parts named by an href are pulled in; this pattern is
       here to prove the others are left behind. -->
  <pattern id="unused">
    <rule context="invoice">
      <assert test="false()">the unused pattern must not be spliced</assert>
    </rule>
  </pattern>

  <pattern id="totals">
    <rule context="invoice">
      <assert test="total">needs a total</assert>
    </rule>
  </pattern>

  <rule id="dated">
    <assert test="@date">needs a date</assert>
    <assert test="@id">needs an id</assert>
  </rule>
</schema>
