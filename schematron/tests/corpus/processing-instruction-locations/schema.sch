<schema xmlns="http://purl.oclc.org/dsdl/schematron">
  <pattern>
    <rule context="processing-instruction()">
      <assert test="false()">pi <value-of select="name()"/> <value-of select="."/></assert>
    </rule>
  </pattern>
</schema>
