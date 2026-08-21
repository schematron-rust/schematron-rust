<schema xmlns="http://purl.oclc.org/dsdl/schematron">
  <let name="base" value="10"/>
  <pattern>
    <let name="limit" value="$base * 2"/>
    <rule context="a">
      <let name="n" value="number(@n)"/>
      <assert test="$n &lt;= $limit">
        <value-of select="$n"/> exceeds <value-of select="$limit"/>
      </assert>
    </rule>
  </pattern>
</schema>
