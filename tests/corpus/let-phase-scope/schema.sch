<schema xmlns="http://purl.oclc.org/dsdl/schematron">
  <let name="base" value="10"/>
  <phase id="strict">
    <let name="base" value="100"/>
    <active pattern="limits"/>
  </phase>
  <pattern id="limits">
    <rule context="a">
      <assert test="false()">limit is <value-of select="$base"/></assert>
    </rule>
  </pattern>
</schema>
