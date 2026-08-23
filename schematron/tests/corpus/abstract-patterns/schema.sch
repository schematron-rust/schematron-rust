<schema xmlns="http://purl.oclc.org/dsdl/schematron">
  <pattern abstract="true" id="required-child">
    <rule context="$parent">
      <assert test="$child">A <name/> must contain a <value-of select="'$child'"/>.</assert>
    </rule>
  </pattern>
  <pattern is-a="required-child" id="invoice-total">
    <param name="parent" value="invoice"/>
    <param name="child" value="total"/>
  </pattern>
  <pattern is-a="required-child" id="order-date">
    <param name="parent" value="order"/>
    <param name="child" value="date"/>
  </pattern>
</schema>
