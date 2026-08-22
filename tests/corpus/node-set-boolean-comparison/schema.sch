<schema xmlns="http://purl.oclc.org/dsdl/schematron">
  <pattern>
    <rule context="a">
      <!-- XPath 1.0 section 3.4: a node-set compared to a boolean is
           converted with boolean() first, and the two booleans are then
           compared as numbers. The nodes' string values never enter into
           it, and an empty node-set is false rather than "no nodes, so
           nothing satisfies the comparison". -->
      <report test="missing &gt;= false()">empty node-set >= false() is 0 >= 0</report>
      <report test="missing &lt; true()">empty node-set &lt; true() is 0 &lt; 1</report>
      <report test="missing &gt; false()">this must not fire: 0 > 0</report>
      <report test="present &gt;= true()">non-empty >= true() is 1 >= 1</report>
      <report test="present &lt; true()">this must not fire: 1 &lt; 1</report>

      <!-- Contrast: against a string or a number the rule *is* existential,
           so an empty node-set makes both the comparison and its negation
           false. -->
      <report test="missing = 'x'">this must not fire: empty = 'x'</report>
      <report test="missing != 'x'">this must not fire: empty != 'x'</report>
    </rule>
  </pattern>
</schema>
