<schema xmlns="http://purl.oclc.org/dsdl/schematron">
  <let name="x" value="'schema'"/>
  <pattern>
    <let name="x" value="'pattern'"/>
    <rule context="a">
      <let name="x" value="'rule'"/>
      <assert test="false()"><value-of select="$x"/></assert>
    </rule>
  </pattern>
</schema>
