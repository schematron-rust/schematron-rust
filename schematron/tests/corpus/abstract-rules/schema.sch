<schema xmlns="http://purl.oclc.org/dsdl/schematron">
  <pattern>
    <rule abstract="true" id="dated">
      <assert test="@date">needs a date</assert>
    </rule>
    <rule abstract="true" id="identified">
      <extends rule="dated"/>
      <assert test="@id">needs an id</assert>
    </rule>
    <rule context="invoice">
      <assert test="total">needs a total</assert>
      <extends rule="identified"/>
      <assert test="line">needs a line</assert>
    </rule>
  </pattern>
</schema>
