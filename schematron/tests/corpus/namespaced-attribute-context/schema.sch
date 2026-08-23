<schema xmlns="http://purl.oclc.org/dsdl/schematron">
  <ns prefix="p" uri="http://example.com/p"/>
  <pattern>
    <!-- An unprefixed name test names the *no namespace* name, for
         attributes exactly as for elements. `@x` is therefore not `@p:x`,
         and a rule on one must not claim the other. -->
    <rule context="@x">
      <report test="true()">@x claims <value-of select="name()"/> on <value-of select="name(parent::*)"/></report>
    </rule>
    <rule context="@p:x">
      <report test="true()">@p:x claims <value-of select="name()"/> on <value-of select="name(parent::*)"/></report>
    </rule>
  </pattern>
  <pattern>
    <!-- The same distinction, counted rather than matched. -->
    <rule context="root">
      <report test="count(//@x) = 2">two attributes named x in no namespace</report>
      <report test="count(//@p:x) = 2">two attributes named x in the p namespace</report>
    </rule>
  </pattern>
</schema>
