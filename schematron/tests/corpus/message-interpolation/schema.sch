<schema xmlns="http://purl.oclc.org/dsdl/schematron">
  <pattern>
    <rule context="line">
      <assert test="false()">
        <name/> number <value-of select="count(preceding-sibling::line) + 1"/>
        has qty <value-of select="@qty"/> and first child <name path="*[1]"/>
      </assert>
    </rule>
  </pattern>
</schema>
